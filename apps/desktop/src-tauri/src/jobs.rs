//! The job manager: the queue, its record in SQLite, and the only code that
//! drives the engines.
//!
//! SQLite is the truth. aria2 and yt-dlp are told about a job only while it
//! holds one of the running slots, and are asked again after a relaunch or a
//! crash; both pick up the partial file they left behind.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;
use url::Url;

use crate::engine::aria2::Aria2Engine;
use crate::engine::ytdlp::YtDlpEngine;
use crate::engine::{
    AddRequest, BrowserContext, Engine, EngineError, JobState, JobStatus, Secret,
    DEFAULT_CONNECTIONS, MAX_CONNECTIONS,
};

/// How many times a download that lost its network is put back in the queue
/// before it is left as failed for the user to retry.
const MAX_AUTO_RETRIES: u32 = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    File,
    Video,
}

impl Kind {
    fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Video => "video",
        }
    }

    fn parse(text: &str) -> Self {
        if text == "video" {
            Self::Video
        } else {
            Self::File
        }
    }
}

fn state_str(state: JobState) -> &'static str {
    match state {
        JobState::Queued => "queued",
        JobState::Active => "active",
        JobState::Paused => "paused",
        JobState::Done => "done",
        JobState::Failed => "failed",
    }
}

fn parse_state(text: &str) -> JobState {
    match text {
        "active" => JobState::Active,
        "paused" => JobState::Paused,
        "done" => JobState::Done,
        "failed" => JobState::Failed,
        _ => JobState::Queued,
    }
}

/// A job as the window sees it. Nothing secret is in here.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobView {
    pub id: i64,
    pub kind: Kind,
    pub state: JobState,
    pub name: String,
    pub dir: PathBuf,
    pub total_bytes: u64,
    pub completed_bytes: u64,
    pub speed_bps: u64,
    pub connections: u32,
    pub error: Option<String>,
    /// Seconds since 1970.
    pub created_at: i64,
    pub finished_at: Option<i64>,
}

impl JobView {
    /// Where the finished file is.
    pub fn path(&self) -> PathBuf {
        self.dir.join(&self.name)
    }
}

/// A job as stored, including what is needed to start it again.
struct Job {
    view: JobView,
    url: String,
    /// The exact file name to save as, once known. Passing it back to the
    /// engine after a relaunch is what makes it resume the same file.
    filename: Option<String>,
    wanted_connections: u8,
    context: BrowserContext,
}

/// What is known about a download when it is asked for.
pub struct NewJob {
    pub kind: Kind,
    pub url: Url,
    /// `None` saves into the default folder.
    pub dir: Option<PathBuf>,
    pub filename: Option<String>,
    pub connections: Option<u8>,
    pub context: BrowserContext,
}

/// What an engine currently knows about one job.
struct Live {
    engine_id: String,
    speed_bps: u64,
    connections: u32,
}

pub struct Config {
    pub db_path: PathBuf,
    pub default_dir: PathBuf,
    /// How many downloads run at once.
    pub max_concurrent: usize,
    /// How long a download that lost its network waits before it is tried again.
    pub retry_delay: Duration,
}

pub struct Manager {
    db: Mutex<Connection>,
    files: Option<Arc<Aria2Engine>>,
    videos: Mutex<Option<Arc<YtDlpEngine>>>,
    live: Mutex<HashMap<i64, Live>>,
    /// Jobs waiting out a lost network: when to try again, and which try it is.
    retries: Mutex<HashMap<i64, (Instant, u32)>>,
    default_dir: PathBuf,
    max_concurrent: usize,
    retry_delay: Duration,
}

type DbResult<T> = Result<T, String>;

fn db_err(error: rusqlite::Error) -> String {
    format!("The download list could not be saved: {error}")
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

const COLUMNS: &str = "id, kind, url, dir, name, filename, state, total_bytes, completed_bytes, \
                       connections, error, referrer, user_agent, cookies, created_at, finished_at";

fn read_job(row: &rusqlite::Row<'_>) -> rusqlite::Result<Job> {
    Ok(Job {
        view: JobView {
            id: row.get(0)?,
            kind: Kind::parse(&row.get::<_, String>(1)?),
            dir: PathBuf::from(row.get::<_, String>(3)?),
            name: row.get(4)?,
            state: parse_state(&row.get::<_, String>(6)?),
            total_bytes: row.get::<_, i64>(7)? as u64,
            completed_bytes: row.get::<_, i64>(8)? as u64,
            speed_bps: 0,
            connections: 0,
            error: row.get(10)?,
            created_at: row.get(14)?,
            finished_at: row.get(15)?,
        },
        url: row.get(2)?,
        filename: row.get(5)?,
        wanted_connections: row.get::<_, i64>(9)? as u8,
        context: BrowserContext {
            referrer: row.get(11)?,
            user_agent: row.get(12)?,
            cookies: row.get::<_, Option<String>>(13)?.map(Secret::new),
        },
    })
}

impl Manager {
    /// Opens the job list. Anything that was running when the app last stopped
    /// goes back into the queue and resumes from its partial file.
    pub fn open(
        config: Config,
        files: Option<Arc<Aria2Engine>>,
        videos: Option<Arc<YtDlpEngine>>,
    ) -> DbResult<Self> {
        if let Some(dir) = config.db_path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let db = Connection::open(&config.db_path).map_err(db_err)?;
        db.pragma_update(None, "journal_mode", "WAL")
            .map_err(db_err)?;
        db.execute_batch(
            "CREATE TABLE IF NOT EXISTS jobs (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                url TEXT NOT NULL,
                dir TEXT NOT NULL,
                name TEXT NOT NULL,
                filename TEXT,
                state TEXT NOT NULL,
                total_bytes INTEGER NOT NULL DEFAULT 0,
                completed_bytes INTEGER NOT NULL DEFAULT 0,
                connections INTEGER NOT NULL,
                error TEXT,
                referrer TEXT,
                user_agent TEXT,
                cookies TEXT,
                created_at INTEGER NOT NULL,
                finished_at INTEGER
            );
            UPDATE jobs SET state = 'queued' WHERE state = 'active';",
        )
        .map_err(db_err)?;

        Ok(Self {
            db: Mutex::new(db),
            files,
            videos: Mutex::new(videos),
            live: Mutex::default(),
            retries: Mutex::default(),
            default_dir: config.default_dir,
            max_concurrent: config.max_concurrent.max(1),
            retry_delay: config.retry_delay,
        })
    }

    pub fn default_dir(&self) -> &std::path::Path {
        &self.default_dir
    }

    pub fn file_engine(&self) -> Option<&Arc<Aria2Engine>> {
        self.files.as_ref()
    }

    pub fn has_video_engine(&self) -> bool {
        self.videos.lock().expect("videos lock").is_some()
    }

    pub fn set_video_engine(&self, engine: Arc<YtDlpEngine>) {
        self.videos
            .lock()
            .expect("videos lock")
            .get_or_insert(engine);
    }

    fn video_engine(&self) -> Option<Arc<YtDlpEngine>> {
        self.videos.lock().expect("videos lock").clone()
    }

    // ------------------------------------------------------------ storage

    fn with_db<T>(&self, work: impl FnOnce(&Connection) -> rusqlite::Result<T>) -> DbResult<T> {
        work(&self.db.lock().expect("db lock")).map_err(db_err)
    }

    fn load(&self, id: i64) -> DbResult<Option<Job>> {
        self.with_db(|db| {
            db.query_row(
                &format!("SELECT {COLUMNS} FROM jobs WHERE id = ?1"),
                [id],
                read_job,
            )
            .optional()
        })
    }

    fn load_all(&self) -> DbResult<Vec<Job>> {
        self.with_db(|db| {
            let mut query = db.prepare(&format!("SELECT {COLUMNS} FROM jobs ORDER BY id"))?;
            let rows = query.query_map([], read_job)?;
            rows.collect()
        })
    }

    fn set_state(&self, id: i64, state: JobState, error: Option<&str>) -> DbResult<()> {
        self.with_db(|db| {
            db.execute(
                "UPDATE jobs SET state = ?2, error = ?3 WHERE id = ?1",
                params![id, state_str(state), error],
            )
        })
        .map(drop)
    }

    /// Every job, oldest first, with live speed where there is one.
    pub fn snapshot(&self) -> DbResult<Vec<JobView>> {
        let jobs = self.load_all()?;
        let live = self.live.lock().expect("live lock");
        Ok(jobs
            .into_iter()
            .map(|job| {
                let mut view = job.view;
                if let (JobState::Active, Some(live)) = (view.state, live.get(&view.id)) {
                    view.speed_bps = live.speed_bps;
                    view.connections = live.connections;
                }
                view
            })
            .collect())
    }

    // ----------------------------------------------------------- commands

    /// Puts a download in the queue and returns its id. It starts on the next
    /// [`Manager::tick`] if a slot is free.
    pub fn add(&self, job: NewJob) -> DbResult<i64> {
        let dir = job.dir.unwrap_or_else(|| self.default_dir.clone());
        let name = job
            .filename
            .clone()
            .unwrap_or_else(|| placeholder_name(&job.url));
        let connections = job
            .connections
            .unwrap_or(DEFAULT_CONNECTIONS)
            .clamp(1, MAX_CONNECTIONS);
        self.with_db(|db| {
            db.execute(
                "INSERT INTO jobs (kind, url, dir, name, filename, state, connections,
                                   referrer, user_agent, cookies, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, 'queued', ?6, ?7, ?8, ?9, ?10)",
                params![
                    job.kind.as_str(),
                    job.url.as_str(),
                    dir.to_string_lossy(),
                    name,
                    job.filename,
                    connections,
                    job.context.referrer,
                    job.context.user_agent,
                    job.context.cookies.as_ref().map(Secret::expose),
                    now(),
                ],
            )?;
            Ok(db.last_insert_rowid())
        })
    }

    pub async fn pause(&self, id: i64) -> DbResult<()> {
        let Some(job) = self.load(id)? else {
            return Ok(());
        };
        match job.view.state {
            JobState::Active => {
                if let Some(engine_id) = self.engine_id(id) {
                    self.engine_pause(job.view.kind, &engine_id)
                        .await
                        .map_err(|e| e.to_string())?;
                }
                self.set_state(id, JobState::Paused, None)
            }
            JobState::Queued => {
                self.retries.lock().expect("retries lock").remove(&id);
                self.set_state(id, JobState::Paused, None)
            }
            _ => Ok(()),
        }
    }

    /// Puts a paused job back in the queue.
    pub fn resume(&self, id: i64) -> DbResult<()> {
        match self.load(id)? {
            Some(job) if job.view.state == JobState::Paused => {
                self.set_state(id, JobState::Queued, None)
            }
            _ => Ok(()),
        }
    }

    /// Puts a failed job back in the queue, to start again from what it has.
    pub fn retry(&self, id: i64) -> DbResult<()> {
        match self.load(id)? {
            Some(job) if job.view.state == JobState::Failed => {
                self.retries.lock().expect("retries lock").remove(&id);
                self.set_state(id, JobState::Queued, None)
            }
            _ => Ok(()),
        }
    }

    /// Stops a job and takes it off the list. Files on disk are left alone.
    pub async fn remove(&self, id: i64) -> DbResult<()> {
        if let Some(job) = self.load(id)? {
            self.forget_in_engine(job.view.kind, id).await;
        }
        self.retries.lock().expect("retries lock").remove(&id);
        self.with_db(|db| db.execute("DELETE FROM jobs WHERE id = ?1", [id]))
            .map(drop)
    }

    pub async fn pause_all(&self) -> DbResult<()> {
        for job in self.load_all()? {
            if matches!(job.view.state, JobState::Active | JobState::Queued) {
                // A job that cannot pause (a video, for now) keeps running.
                let _ = self.pause(job.view.id).await;
            }
        }
        Ok(())
    }

    pub fn resume_all(&self) -> DbResult<()> {
        self.with_db(|db| {
            db.execute(
                "UPDATE jobs SET state = 'queued' WHERE state = 'paused'",
                [],
            )
        })
        .map(drop)
    }

    // ------------------------------------------------------------ engines

    fn engine_id(&self, id: i64) -> Option<String> {
        self.live
            .lock()
            .expect("live lock")
            .get(&id)
            .map(|live| live.engine_id.clone())
    }

    async fn engine_status(&self, kind: Kind, engine_id: &str) -> Result<JobStatus, EngineError> {
        match kind {
            Kind::File => match &self.files {
                Some(engine) => engine.status(engine_id).await,
                None => Err(EngineError::Transport(
                    "the file engine is not running".into(),
                )),
            },
            Kind::Video => match self.video_engine() {
                Some(engine) => engine.status(engine_id).await,
                None => Err(EngineError::Rejected("the video tools are missing".into())),
            },
        }
    }

    async fn engine_pause(&self, kind: Kind, engine_id: &str) -> Result<(), EngineError> {
        match kind {
            Kind::File => match &self.files {
                Some(engine) => engine.pause(engine_id).await,
                None => Ok(()),
            },
            Kind::Video => match self.video_engine() {
                Some(engine) => engine.pause(engine_id).await,
                None => Ok(()),
            },
        }
    }

    /// Makes the engine drop whatever it holds for this job.
    async fn forget_in_engine(&self, kind: Kind, id: i64) {
        let Some(live) = self.live.lock().expect("live lock").remove(&id) else {
            return;
        };
        match kind {
            Kind::File => {
                if let Some(engine) = &self.files {
                    let _ = engine.remove(&live.engine_id).await;
                }
            }
            Kind::Video => {
                if let Some(engine) = self.video_engine() {
                    let _ = engine.remove(&live.engine_id).await;
                }
            }
        }
    }

    /// Gives a queued job to its engine: wakes it if the engine still holds it
    /// paused, otherwise adds it, which resumes any partial file.
    async fn start(&self, job: &Job) -> Result<(), EngineError> {
        let id = job.view.id;
        if let Some(engine_id) = self.engine_id(id) {
            let woken = match (job.view.kind, &self.files, self.video_engine()) {
                (Kind::File, Some(engine), _) => engine.resume(&engine_id).await,
                (Kind::Video, _, Some(engine)) => engine.resume(&engine_id).await,
                _ => Err(EngineError::Rejected("no engine".into())),
            };
            if woken.is_ok() {
                return Ok(());
            }
            self.forget_in_engine(job.view.kind, id).await;
        }

        let url = Url::parse(&job.url).map_err(|e| EngineError::Rejected(e.to_string()))?;
        let request = AddRequest {
            url,
            dir: job.view.dir.clone(),
            connections: job.wanted_connections,
            filename: job.filename.clone(),
            context: job.context.clone(),
        };
        let engine_id = match job.view.kind {
            Kind::File => match &self.files {
                Some(engine) => engine.add(request).await?,
                None => {
                    return Err(EngineError::Start(
                        crate::strings::ENGINE_NOT_RUNNING.into(),
                    ))
                }
            },
            Kind::Video => match self.video_engine() {
                Some(engine) => engine.add(request).await?,
                None => return Err(EngineError::Start(crate::strings::TOOLS_MISSING.into())),
            },
        };
        self.live.lock().expect("live lock").insert(
            id,
            Live {
                engine_id,
                speed_bps: 0,
                connections: 0,
            },
        );
        Ok(())
    }

    // --------------------------------------------------------------- tick

    /// One turn of the crank, once a second: learn what the engines did, record
    /// it, and fill free slots from the queue. Returns the jobs that finished.
    pub async fn tick(&self) -> DbResult<Vec<JobView>> {
        let mut finished = Vec::new();
        let mut file_engine_down = false;

        for job in self.load_all()? {
            if job.view.state != JobState::Active {
                continue;
            }
            let id = job.view.id;
            let Some(engine_id) = self.engine_id(id) else {
                self.set_state(id, JobState::Queued, None)?;
                continue;
            };
            match self.engine_status(job.view.kind, &engine_id).await {
                Ok(status) => {
                    if let Some(done) = self.record(&job, &status).await? {
                        finished.push(done);
                    }
                }
                Err(EngineError::Transport(_)) if job.view.kind == Kind::File => {
                    file_engine_down = true;
                }
                // The engine no longer knows the job: ask it again.
                Err(_) => {
                    self.live.lock().expect("live lock").remove(&id);
                    self.set_state(id, JobState::Queued, None)?;
                }
            }
        }

        if file_engine_down {
            self.recover_file_engine().await?;
        }
        self.fill_slots().await?;
        Ok(finished)
    }

    /// Writes one status report into the job's row. Returns the job if it just finished.
    async fn record(&self, job: &Job, status: &JobStatus) -> DbResult<Option<JobView>> {
        let id = job.view.id;
        let name = if status.name.is_empty() {
            job.view.name.clone()
        } else {
            status.name.clone()
        };
        // A file's real name is fixed the moment aria2 has chosen it, so a
        // relaunch asks for exactly that file again.
        let filename = match (job.view.kind, &job.filename) {
            (Kind::File, None) if !status.name.is_empty() && status.total_bytes > 0 => {
                Some(status.name.clone())
            }
            (_, existing) => existing.clone(),
        };
        self.with_db(|db| {
            db.execute(
                "UPDATE jobs SET name = ?2, filename = ?3, total_bytes = ?4, completed_bytes = ?5
                 WHERE id = ?1",
                params![
                    id,
                    name,
                    filename,
                    status.total_bytes as i64,
                    status.completed_bytes as i64
                ],
            )
        })?;
        if let Some(live) = self.live.lock().expect("live lock").get_mut(&id) {
            live.speed_bps = status.speed_bps;
            live.connections = status.connections;
        }

        match status.state {
            JobState::Done => {
                self.forget_in_engine(job.view.kind, id).await;
                self.retries.lock().expect("retries lock").remove(&id);
                // The job is finished: its cookies and headers are deleted now.
                self.with_db(|db| {
                    db.execute(
                        "UPDATE jobs SET state = 'done', error = NULL, finished_at = ?2,
                                         cookies = NULL, referrer = NULL, user_agent = NULL
                         WHERE id = ?1",
                        params![id, now()],
                    )
                })?;
                Ok(self.load(id)?.map(|job| job.view))
            }
            JobState::Failed => {
                self.forget_in_engine(job.view.kind, id).await;
                let attempt = {
                    let retries = self.retries.lock().expect("retries lock");
                    retries.get(&id).map_or(0, |(_, attempt)| *attempt)
                };
                if status.retryable && attempt < MAX_AUTO_RETRIES {
                    let again = (Instant::now() + self.retry_delay, attempt + 1);
                    self.retries.lock().expect("retries lock").insert(id, again);
                    self.set_state(id, JobState::Queued, status.error.as_deref())?;
                } else {
                    self.set_state(id, JobState::Failed, status.error.as_deref())?;
                }
                Ok(None)
            }
            _ => Ok(None),
        }
    }

    /// aria2 died. Start another and send every running file back through the queue.
    async fn recover_file_engine(&self) -> DbResult<()> {
        let Some(engine) = &self.files else {
            return Ok(());
        };
        if !matches!(engine.restart_if_dead().await, Ok(true)) {
            return Ok(());
        }
        for job in self.load_all()? {
            if job.view.kind == Kind::File {
                // The new aria2 knows none of the old ids, paused ones included.
                self.live.lock().expect("live lock").remove(&job.view.id);
                if job.view.state == JobState::Active {
                    self.set_state(job.view.id, JobState::Queued, None)?;
                }
            }
        }
        Ok(())
    }

    async fn fill_slots(&self) -> DbResult<()> {
        let jobs = self.load_all()?;
        let mut running = jobs
            .iter()
            .filter(|job| job.view.state == JobState::Active)
            .count();
        for job in jobs.iter().filter(|job| job.view.state == JobState::Queued) {
            if running >= self.max_concurrent {
                break;
            }
            let waiting = {
                let retries = self.retries.lock().expect("retries lock");
                retries
                    .get(&job.view.id)
                    .is_some_and(|(at, _)| *at > Instant::now())
            };
            if waiting {
                continue;
            }
            match self.start(job).await {
                Ok(()) => {
                    self.set_state(job.view.id, JobState::Active, None)?;
                    running += 1;
                }
                Err(error) => {
                    self.set_state(job.view.id, JobState::Failed, Some(&error.to_string()))?;
                }
            }
        }
        Ok(())
    }
}

/// What to call a download before the engine has told us its real name.
fn placeholder_name(url: &Url) -> String {
    let last = url
        .path_segments()
        .and_then(|mut parts| parts.next_back())
        .unwrap_or_default();
    if last.is_empty() {
        url.host_str().unwrap_or("download").to_owned()
    } else {
        last.to_owned()
    }
}
