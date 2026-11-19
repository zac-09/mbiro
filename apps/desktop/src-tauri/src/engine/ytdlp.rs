//! yt-dlp as an [`Engine`]: one `yt-dlp` process per job, started with plain
//! command-line arguments and followed through the lines it prints.
//!
//! There is no site-specific code here and there never will be: yt-dlp decides
//! what a page is. Protected streams are refused by yt-dlp itself, and nothing
//! here asks it to try.
//!
//! Pausing stops yt-dlp and resuming starts it again; it continues from the
//! partial files it left.

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
    /// What was asked for, kept so the job can be started again after a pause.
    request: AddRequest,
    /// The task that owns the running yt-dlp. Stopping the task kills yt-dlp;
    /// `None` while paused or once finished.
    task: Option<JoinHandle<()>>,
}

type Jobs = Arc<Mutex<HashMap<String, Job>>>;

pub struct YtDlpEngine {
    /// Replaceable: yt-dlp is updated while the app runs.
    tools: std::sync::RwLock<ToolPaths>,
    jobs: Jobs,
    next: AtomicU64,
}

impl YtDlpEngine {
    pub fn tools(&self) -> ToolPaths {
        self.tools.read().expect("tools lock").clone()
    }

    /// Points new downloads at a newer yt-dlp. Ones already running finish on the old.
    pub fn set_yt_dlp(&self, path: PathBuf) {
        self.tools.write().expect("tools lock").yt_dlp = path;
    }

    pub fn new(tools: ToolPaths) -> Self {
        Self {
            tools: std::sync::RwLock::new(tools),
            jobs: Jobs::default(),
            next: AtomicU64::new(1),
        }
    }

    fn command(&self, request: &AddRequest) -> Command {
        let tools = self.tools();
        let mut command = Command::new(&tools.yt_dlp);
        match &request.video {
            Some(video) if video.audio_only => {
                command.args(["--format", &video.format, "--extract-audio"]);
            }
            Some(video) => {
                command.args(["--format", &video.format, "--merge-output-format", "mp4"]);
            }
            // Best video with best audio, merged; else the best single file.
            None => {
                command.args(["--format", "bv*+ba/b", "--merge-output-format", "mp4"]);
            }
        }
        command
            .arg("--ignore-config")
            .arg("--no-playlist")
            .arg("--js-runtimes")
            .arg(format!("deno:{}", tools.deno.display()))
            .arg("--ffmpeg-location")
            .arg(&tools.ffmpeg)
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

impl YtDlpEngine {
    /// Starts yt-dlp for a job and follows it. The job must already be in the map,
    /// or be put there before the caller releases the lock it holds.
    fn launch(&self, id: &str, request: &AddRequest) -> Result<JoinHandle<()>, EngineError> {
        let mut child = self
            .command(request)
            .spawn()
            .map_err(|e| EngineError::Start(format!("could not launch yt-dlp: {e}")))?;
        let stdout = child.stdout.take().expect("stdout is piped");
        let stderr = child.stderr.take().expect("stderr is piped");

        let (jobs, job_id) = (self.jobs.clone(), id.to_owned());
        let update = move |change: &dyn Fn(&mut Job)| {
            if let Some(job) = jobs.lock().expect("jobs lock").get_mut(&job_id) {
                change(job);
            }
        };

        Ok(tokio::spawn(async move {
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
                    Some(Event::Name(name)) => update(&|job| job.status.name = name.clone()),
                    Some(Event::Progress { done, total, speed }) => update(&|job| {
                        job.status.completed_bytes = done;
                        job.status.total_bytes = total.max(done);
                        job.status.speed_bps = speed;
                    }),
                    Some(Event::Done(path)) => update(&|job| {
                        if let Some(name) = path.file_name() {
                            job.status.name = name.to_string_lossy().into_owned();
                        }
                    }),
                    None => {}
                }
            }

            let succeeded = child.wait().await.map(|s| s.success()).unwrap_or(false);
            let reason = errors.await.ok().flatten();
            update(&|job| {
                job.task = None;
                let s = &mut job.status;
                s.speed_bps = 0;
                s.connections = 0;
                if succeeded {
                    s.state = JobState::Done;
                    s.total_bytes = s.total_bytes.max(s.completed_bytes);
                    s.completed_bytes = s.total_bytes;
                } else {
                    s.state = JobState::Failed;
                    s.error = Some(failure_message(reason.as_deref()).to_owned());
                }
            });
        }))
    }
}

impl Engine for YtDlpEngine {
    async fn add(&self, request: AddRequest) -> Result<String, EngineError> {
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
        // Hold the map while yt-dlp starts, so its first report finds the job.
        let mut map = self.jobs.lock().expect("jobs lock");
        let task = Some(self.launch(&id, &request)?);
        map.insert(
            id.clone(),
            Job {
                status,
                request,
                task,
            },
        );
        Ok(id)
    }

    /// yt-dlp cannot be told to wait, so pausing stops it. It leaves its
    /// partial files behind and picks them up when it is started again.
    async fn pause(&self, id: &str) -> Result<(), EngineError> {
        let mut map = self.jobs.lock().expect("jobs lock");
        let job = map
            .get_mut(id)
            .ok_or_else(|| EngineError::Rejected("no such download".into()))?;
        if job.status.state == JobState::Active {
            if let Some(task) = job.task.take() {
                task.abort();
            }
            job.status.state = JobState::Paused;
            job.status.speed_bps = 0;
            job.status.connections = 0;
        }
        Ok(())
    }

    async fn resume(&self, id: &str) -> Result<(), EngineError> {
        let mut map = self.jobs.lock().expect("jobs lock");
        let job = map
            .get_mut(id)
            .ok_or_else(|| EngineError::Rejected("no such download".into()))?;
        if job.status.state == JobState::Paused {
            job.task = Some(self.launch(id, &job.request)?);
            job.status.state = JobState::Active;
            job.status.connections = 1;
        }
        Ok(())
    }

    async fn remove(&self, id: &str) -> Result<(), EngineError> {
        match self.jobs.lock().expect("jobs lock").remove(id) {
            Some(job) => {
                if let Some(task) = job.task {
                    task.abort();
                }
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

/// A plain sentence for a yt-dlp failure. Its own text is never shown: it names
/// the site and suggests command-line options, neither of which belongs in the app.
pub fn failure_message(raw: Option<&str>) -> &'static str {
    use crate::strings::*;
    let text = raw.unwrap_or_default().to_ascii_lowercase();
    let has = |words: &[&str]| words.iter().any(|word| text.contains(word));
    if has(&["drm"]) {
        VIDEO_FAIL_PROTECTED
    } else if has(&[
        "sign in",
        "log in",
        "login",
        "cookies",
        "members-only",
        "premium",
    ]) {
        VIDEO_FAIL_SIGN_IN
    } else if has(&["unsupported url"]) {
        VIDEO_PROBE_FAILED
    } else if has(&[
        "unavailable",
        "private",
        "removed",
        "not available",
        "does not exist",
    ]) {
        VIDEO_FAIL_UNAVAILABLE
    } else {
        VIDEO_FAIL_OTHER
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

    #[test]
    fn failures_become_plain_sentences_that_name_no_site() {
        use crate::strings::*;
        let message = |raw| failure_message(Some(raw));
        assert_eq!(
            message("[somesite] abc: This video is DRM protected"),
            VIDEO_FAIL_PROTECTED
        );
        assert_eq!(
            message("[somesite] abc: Sign in to confirm your age. Use --cookies-from-browser"),
            VIDEO_FAIL_SIGN_IN
        );
        assert_eq!(
            message("Unsupported URL: https://example.com/"),
            VIDEO_PROBE_FAILED
        );
        assert_eq!(
            message("[somesite] abc: Video unavailable"),
            VIDEO_FAIL_UNAVAILABLE
        );
        assert_eq!(message("[somesite] abc: something new"), VIDEO_FAIL_OTHER);
        assert_eq!(failure_message(None), VIDEO_FAIL_OTHER);
    }
}
