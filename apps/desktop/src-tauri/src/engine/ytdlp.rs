//! yt-dlp as an [`Engine`]: one `yt-dlp` process per job, started with plain
//! command-line arguments and followed through the lines it prints.
//!
//! There is no site-specific code here and there never will be: yt-dlp decides
//! what a page is. Protected streams are refused by yt-dlp itself, and nothing
//! here asks it to try.
//!
//! Spike 3 scope: start, follow, stop. Pause and resume, the quality picker and
//! playlists come in M3.

use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::task::JoinHandle;

use super::{AddRequest, Engine, EngineError, JobState, JobStatus};

/// Where the fetched tools are. yt-dlp is told about the other two explicitly:
/// an app started from the Finder has no useful `PATH`.
#[derive(Debug, Clone)]
pub struct ToolPaths {
    pub yt_dlp: PathBuf,
    pub deno: PathBuf,
    pub ffmpeg: PathBuf,
}

struct Job {
    status: JobStatus,
    /// Dropping the task drops the child, which kills it.
    task: JoinHandle<()>,
}

type Jobs = Arc<Mutex<HashMap<String, Job>>>;

pub struct YtDlpEngine {
    tools: ToolPaths,
    jobs: Jobs,
    next: AtomicU64,
}

impl YtDlpEngine {
    pub fn new(tools: ToolPaths) -> Self {
        Self {
            tools,
            jobs: Jobs::default(),
            next: AtomicU64::new(1),
        }
    }

    fn command(&self, request: &AddRequest) -> Command {
        let mut command = Command::new(&self.tools.yt_dlp);
        command
            .arg("--ignore-config")
            .arg("--no-playlist")
            .arg("--js-runtimes")
            .arg(format!("deno:{}", self.tools.deno.display()))
            .arg("--ffmpeg-location")
            .arg(&self.tools.ffmpeg)
            // Best video with best audio, merged; else the best single file.
            .args(["--format", "bv*+ba/b", "--merge-output-format", "mp4"])
            .arg("--paths")
            .arg(&request.dir)
            .args(["--output", "%(title).200B.%(ext)s"])
            // One machine-readable line per event, nothing else on stdout.
            .args([
                "--quiet",
                "--no-warnings",
                "--progress",
                "--newline",
                "--no-simulate",
            ])
            .args(["--print", "before_dl:NAME %(title)s"])
            .args(["--print", "after_move:DONE %(filepath)s"])
            .arg("--progress-template")
            .arg(
                "download:PROGRESS %(progress.downloaded_bytes)s %(progress.total_bytes)s \
                 %(progress.total_bytes_estimate)s %(progress.speed)s",
            )
            // The URL is data, never an option.
            .arg("--")
            .arg(request.url.as_str())
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        command
    }
}

impl Engine for YtDlpEngine {
    async fn add(&self, request: AddRequest) -> Result<String, EngineError> {
        let mut child = self
            .command(&request)
            .spawn()
            .map_err(|e| EngineError::Start(format!("could not launch yt-dlp: {e}")))?;
        let stdout = child.stdout.take().expect("stdout is piped");
        let stderr = child.stderr.take().expect("stderr is piped");

        let id = format!("video-{}", self.next.fetch_add(1, Ordering::Relaxed));
        let status = JobStatus {
            id: id.clone(),
            state: JobState::Active,
            name: request.url.host_str().unwrap_or_default().to_owned(),
            total_bytes: 0,
            completed_bytes: 0,
            speed_bps: 0,
            connections: 1,
            error: None,
            retryable: false,
        };

        let (jobs, job_id) = (self.jobs.clone(), id.clone());
        let update = move |change: &dyn Fn(&mut JobStatus)| {
            if let Some(job) = jobs.lock().expect("jobs lock").get_mut(&job_id) {
                change(&mut job.status);
            }
        };

        // Hold the map while the task starts, so its first update finds the job.
        let mut map = self.jobs.lock().expect("jobs lock");
        let task = tokio::spawn(async move {
            let errors = tokio::spawn(async move {
                let mut last = None;
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    if let Some(reason) = line.strip_prefix("ERROR: ") {
                        last = Some(reason.to_owned());
                    }
                }
                last
            });

            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                match parse_line(&line) {
                    Some(Event::Name(name)) => update(&|s| s.name = name.clone()),
                    Some(Event::Progress { done, total, speed }) => update(&|s| {
                        s.completed_bytes = done;
                        s.total_bytes = total.max(done);
                        s.speed_bps = speed;
                    }),
                    Some(Event::Done(path)) => update(&|s| {
                        if let Some(name) = path.file_name() {
                            s.name = name.to_string_lossy().into_owned();
                        }
                    }),
                    None => {}
                }
            }

            let succeeded = child.wait().await.map(|s| s.success()).unwrap_or(false);
            let reason = errors.await.ok().flatten();
            update(&|s| {
                s.speed_bps = 0;
                s.connections = 0;
                if succeeded {
                    s.state = JobState::Done;
                    s.total_bytes = s.total_bytes.max(s.completed_bytes);
                    s.completed_bytes = s.total_bytes;
                } else {
                    s.state = JobState::Failed;
                    s.error = reason.clone().or_else(|| Some("yt-dlp stopped".into()));
                }
            });
        });
        map.insert(id.clone(), Job { status, task });
        Ok(id)
    }

    async fn pause(&self, _id: &str) -> Result<(), EngineError> {
        Err(EngineError::Rejected(
            crate::strings::VIDEO_PAUSE_UNAVAILABLE.into(),
        ))
    }

    async fn resume(&self, _id: &str) -> Result<(), EngineError> {
        Err(EngineError::Rejected(
            crate::strings::VIDEO_PAUSE_UNAVAILABLE.into(),
        ))
    }

    async fn remove(&self, id: &str) -> Result<(), EngineError> {
        match self.jobs.lock().expect("jobs lock").remove(id) {
            Some(job) => {
                job.task.abort();
                Ok(())
            }
            None => Err(EngineError::Rejected("no such download".into())),
        }
    }

    async fn status(&self, id: &str) -> Result<JobStatus, EngineError> {
        self.jobs
            .lock()
            .expect("jobs lock")
            .get(id)
            .map(|job| job.status.clone())
            .ok_or_else(|| EngineError::Rejected("no such download".into()))
    }
}

#[derive(Debug, PartialEq)]
enum Event {
    Name(String),
    Progress { done: u64, total: u64, speed: u64 },
    Done(PathBuf),
}

/// Reads one of the lines the templates in `command` produce. yt-dlp prints
/// `NA` for a number it does not know, and some numbers as decimals.
fn parse_line(line: &str) -> Option<Event> {
    if let Some(name) = line.strip_prefix("NAME ") {
        return Some(Event::Name(name.trim().to_owned()));
    }
    if let Some(path) = line.strip_prefix("DONE ") {
        return Some(Event::Done(PathBuf::from(path.trim())));
    }
    let mut fields = line.strip_prefix("PROGRESS ")?.split_whitespace();
    let mut number = || {
        fields
            .next()
            .and_then(|f| f.parse::<f64>().ok())
            .map(|n| n.max(0.0) as u64)
    };
    let done = number().unwrap_or(0);
    let (total, estimate) = (number(), number());
    let speed = number().unwrap_or(0);
    Some(Event::Progress {
        done,
        total: total.or(estimate).unwrap_or(0),
        speed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_lines_yt_dlp_prints() {
        assert_eq!(
            parse_line("NAME A clip"),
            Some(Event::Name("A clip".into()))
        );
        assert_eq!(
            parse_line("DONE /Users/me/Downloads/A clip.mp4"),
            Some(Event::Done("/Users/me/Downloads/A clip.mp4".into()))
        );
        assert_eq!(
            parse_line("PROGRESS 1048576 4194304 NA 524288.5"),
            Some(Event::Progress {
                done: 1_048_576,
                total: 4_194_304,
                speed: 524_288
            })
        );
        assert_eq!(
            parse_line("PROGRESS 2048 NA 8192.0 NA"),
            Some(Event::Progress {
                done: 2048,
                total: 8192,
                speed: 0
            })
        );
        assert_eq!(parse_line("[download] something else"), None);
    }
}
