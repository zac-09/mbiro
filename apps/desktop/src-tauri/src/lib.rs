//! The Rust core. It owns the engine process and the list of jobs; the window
//! only sends commands and renders the events sent from here.

mod browsers;
mod constants;
pub mod engine;
mod ipc;
mod strings;
pub mod tools;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};
use url::Url;

use engine::aria2::Aria2Engine;
use engine::ytdlp::{ToolPaths, YtDlpEngine};
use engine::{AddRequest, BrowserContext, Engine, JobStatus, Secret, DEFAULT_CONNECTIONS};

const POLL_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
enum EngineHealth {
    Ready { version: String },
    Failed { reason: String },
}

/// Which engine a job belongs to.
#[derive(Clone, Copy, PartialEq)]
enum Kind {
    File,
    Video,
}

#[derive(Clone)]
struct JobRef {
    kind: Kind,
    id: String,
}

struct AppState {
    engine: Option<Arc<Aria2Engine>>,
    health: EngineHealth,
    /// Present once the video tools are installed.
    video: Mutex<Option<Arc<YtDlpEngine>>>,
    tools_dir: PathBuf,
    fetching_tools: AtomicBool,
    download_dir: Option<PathBuf>,
    /// The jobs this run knows about, oldest first.
    jobs: Mutex<Vec<JobRef>>,
    /// What the window was last told.
    snapshot: Mutex<Vec<JobStatus>>,
}

impl AppState {
    fn files(&self) -> Result<Arc<Aria2Engine>, String> {
        self.engine
            .clone()
            .ok_or_else(|| strings::ENGINE_NOT_RUNNING.to_owned())
    }

    fn videos(&self) -> Result<Arc<YtDlpEngine>, String> {
        let video = self.video.lock().expect("video lock").clone();
        video.ok_or_else(|| strings::TOOLS_MISSING.to_owned())
    }

    fn kind_of(&self, id: &str) -> Option<Kind> {
        let jobs = self.jobs.lock().expect("jobs lock");
        jobs.iter().find(|job| job.id == id).map(|job| job.kind)
    }

    fn track(&self, kind: Kind, id: &str) {
        self.jobs.lock().expect("jobs lock").push(JobRef {
            kind,
            id: id.to_owned(),
        });
    }

    /// Creates the video engine if every tool is in place.
    fn enable_video_if_ready(&self) {
        let root = &self.tools_dir;
        if tools::ALL.iter().all(|tool| tool.is_installed(root)) {
            let paths = ToolPaths {
                yt_dlp: tools::YT_DLP.executable_path(root),
                deno: tools::DENO.executable_path(root),
                ffmpeg: tools::FFMPEG.executable_path(root),
            };
            let mut video = self.video.lock().expect("video lock");
            if video.is_none() {
                *video = Some(Arc::new(YtDlpEngine::new(paths)));
            }
        }
    }
}

#[tauri::command]
fn engine_health(state: State<'_, AppState>) -> EngineHealth {
    state.health.clone()
}

#[tauri::command]
fn list_downloads(state: State<'_, AppState>) -> Vec<JobStatus> {
    state.snapshot.lock().expect("snapshot lock").clone()
}

/// Why a download could not be started.
enum AddError {
    InvalidUrl,
    UnsupportedScheme,
    InvalidContext,
    NoDownloadFolder,
    EngineDown,
    Engine(String),
}

impl AddError {
    /// What the window shows.
    fn message(self) -> String {
        match self {
            Self::InvalidUrl => strings::URL_INVALID.to_owned(),
            Self::UnsupportedScheme => strings::URL_SCHEME_UNSUPPORTED.to_owned(),
            Self::InvalidContext => strings::REQUEST_INVALID.to_owned(),
            Self::NoDownloadFolder => strings::DOWNLOAD_FOLDER_MISSING.to_owned(),
            Self::EngineDown => strings::ENGINE_NOT_RUNNING.to_owned(),
            Self::Engine(reason) => reason,
        }
    }

    /// What the extension is told.
    fn code(&self) -> ipc::RefusalCode {
        match self {
            Self::InvalidUrl | Self::UnsupportedScheme => ipc::RefusalCode::UnsupportedUrl,
            Self::InvalidContext => ipc::RefusalCode::InvalidMessage,
            Self::NoDownloadFolder | Self::EngineDown => ipc::RefusalCode::EngineUnavailable,
            Self::Engine(_) => ipc::RefusalCode::Rejected,
        }
    }
}

/// Longest header value accepted from a browser. Real cookie headers stay well below this.
const MAX_HEADER_VALUE: usize = 32 * 1024;

/// A value that can go into an HTTP header as it is: no control characters
/// (so no way to smuggle in a second header) and a sane length.
fn header_safe(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAX_HEADER_VALUE && !value.chars().any(char::is_control)
}

/// Reduces a name suggested by a browser to a plain file name, or nothing.
/// Folders, hidden files and odd characters are never taken from outside.
fn safe_filename(suggested: &str) -> Option<String> {
    let name = suggested.rsplit(['/', '\\']).next()?.trim();
    let plain = !name.is_empty()
        && name.len() <= 255
        && !name.starts_with('.')
        && !name.chars().any(char::is_control);
    plain.then(|| name.to_owned())
}

/// What is known about a download before it is checked.
#[derive(Default)]
struct DownloadSpec {
    url: String,
    filename: Option<String>,
    context: BrowserContext,
}

/// The one way a download starts, whether asked for by the window or a browser.
async fn start_download(state: &AppState, spec: DownloadSpec) -> Result<String, AddError> {
    let url = Url::parse(spec.url.trim()).map_err(|_| AddError::InvalidUrl)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AddError::UnsupportedScheme);
    }
    let BrowserContext {
        referrer,
        user_agent,
        cookies,
    } = spec.context;
    let headers = [
        referrer.as_deref(),
        user_agent.as_deref(),
        cookies.as_ref().map(Secret::expose),
    ];
    if !headers.into_iter().flatten().all(header_safe) {
        return Err(AddError::InvalidContext);
    }
    let dir = state
        .download_dir
        .clone()
        .ok_or(AddError::NoDownloadFolder)?;
    let engine = state.engine.as_ref().ok_or(AddError::EngineDown)?;
    let request = AddRequest {
        url,
        dir,
        connections: DEFAULT_CONNECTIONS,
        filename: spec.filename.as_deref().and_then(safe_filename),
        context: BrowserContext {
            referrer,
            user_agent,
            cookies,
        },
    };
    let id = engine
        .add(request)
        .await
        .map_err(|e| AddError::Engine(e.to_string()))?;
    state.track(Kind::File, &id);
    Ok(id)
}

#[tauri::command]
async fn add_download(state: State<'_, AppState>, url: String) -> Result<String, String> {
    let spec = DownloadSpec {
        url,
        ..Default::default()
    };
    start_download(&state, spec)
        .await
        .map_err(AddError::message)
}

/// Answers the native messaging host for the life of the app.
async fn serve_browsers(app: AppHandle, listener: tokio::net::UnixListener) {
    ipc::serve(listener, move |request| {
        let app = app.clone();
        async move {
            let ipc::Request::Download(request) = request;
            let spec = DownloadSpec {
                url: request.url,
                filename: request.filename,
                context: BrowserContext {
                    referrer: request.referrer,
                    user_agent: request.user_agent,
                    cookies: request.cookies,
                },
            };
            match start_download(&app.state::<AppState>(), spec).await {
                Ok(id) => ipc::Ack::accepted(id),
                Err(error) => ipc::Ack::refused(error.code()),
            }
        }
    })
    .await;
}

/// Opens the socket and points the browsers at the host. Browser capture is
/// optional: if any of this fails, the app still works by paste.
fn start_browser_integration(app: &AppHandle) {
    let Ok(home) = app.path().home_dir() else {
        return;
    };
    if let Ok(host) = sidecar_path(constants::NM_HOST_BINARY) {
        let _ = browsers::install_manifests(&home, &host);
    }
    let socket = mbiro_common::socket_path(&home);
    // Binding needs the runtime, so it happens inside the task.
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        if let Ok(listener) = ipc::bind(&socket) {
            serve_browsers(app, listener).await;
        }
    });
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolStatus {
    name: &'static str,
    version: &'static str,
    installed: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ToolProgress {
    name: &'static str,
    received: u64,
    total: Option<u64>,
}

#[tauri::command]
fn tools_status(state: State<'_, AppState>) -> Vec<ToolStatus> {
    tools::ALL
        .iter()
        .map(|tool| ToolStatus {
            name: tool.name,
            version: tool.version,
            installed: tool.is_installed(&state.tools_dir),
        })
        .collect()
}

/// Fetches whichever video tools are missing. Started only by the user.
#[tauri::command]
async fn fetch_tools(app: AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    if state.fetching_tools.swap(true, Ordering::SeqCst) {
        return Err(strings::TOOLS_BUSY.to_owned());
    }
    let mut outcome = Ok(());
    for tool in tools::ALL {
        let report = |received, total| {
            let _ = app.emit(
                constants::TOOL_PROGRESS_EVENT,
                ToolProgress {
                    name: tool.name,
                    received,
                    total,
                },
            );
        };
        if let Err(error) = tool.install(&state.tools_dir, report).await {
            outcome = Err(match error {
                tools::ToolError::ChecksumMismatch => strings::TOOL_CHECKSUM_FAILED.to_owned(),
                _ => strings::TOOL_FETCH_FAILED.to_owned(),
            });
            break;
        }
    }
    state.fetching_tools.store(false, Ordering::SeqCst);
    state.enable_video_if_ready();
    outcome
}

/// Hands a page address to yt-dlp. Which pages work is yt-dlp's business.
#[tauri::command]
async fn add_video(state: State<'_, AppState>, url: String) -> Result<String, String> {
    let url = Url::parse(url.trim()).map_err(|_| strings::URL_INVALID.to_owned())?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(strings::URL_SCHEME_UNSUPPORTED.to_owned());
    }
    let dir = state
        .download_dir
        .clone()
        .ok_or_else(|| strings::DOWNLOAD_FOLDER_MISSING.to_owned())?;
    let request = AddRequest {
        url,
        dir,
        connections: 1,
        filename: None,
        context: BrowserContext::default(),
    };
    let id = state
        .videos()?
        .add(request)
        .await
        .map_err(|e| e.to_string())?;
    state.track(Kind::Video, &id);
    Ok(id)
}

#[tauri::command]
async fn pause_download(state: State<'_, AppState>, id: String) -> Result<(), String> {
    match state.kind_of(&id) {
        Some(Kind::Video) => state.videos()?.pause(&id).await,
        _ => state.files()?.pause(&id).await,
    }
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn resume_download(state: State<'_, AppState>, id: String) -> Result<(), String> {
    match state.kind_of(&id) {
        Some(Kind::Video) => state.videos()?.resume(&id).await,
        _ => state.files()?.resume(&id).await,
    }
    .map_err(|e| e.to_string())
}

#[tauri::command]
async fn remove_download(state: State<'_, AppState>, id: String) -> Result<(), String> {
    match state.kind_of(&id) {
        Some(Kind::Video) => state.videos()?.remove(&id).await,
        _ => state.files()?.remove(&id).await,
    }
    .map_err(|e| e.to_string())?;
    state
        .jobs
        .lock()
        .expect("jobs lock")
        .retain(|job| job.id != id);
    Ok(())
}

/// Asks the engines about every job once a second and tells the window.
/// Runs for the life of the app, with or without a window.
async fn poll_jobs(app: AppHandle) {
    let mut tick = tokio::time::interval(POLL_INTERVAL);
    loop {
        tick.tick().await;
        let state = app.state::<AppState>();
        let jobs = state.jobs.lock().expect("jobs lock").clone();
        let mut snapshot = Vec::with_capacity(jobs.len());
        for job in &jobs {
            let status = match job.kind {
                Kind::File => match state.files() {
                    Ok(engine) => engine.status(&job.id).await.ok(),
                    Err(_) => None,
                },
                Kind::Video => match state.videos() {
                    Ok(engine) => engine.status(&job.id).await.ok(),
                    Err(_) => None,
                },
            };
            snapshot.extend(status);
        }
        *state.snapshot.lock().expect("snapshot lock") = snapshot.clone();
        let _ = app.emit(constants::SNAPSHOT_EVENT, snapshot);
    }
}

/// Tauri places a sidecar next to the main executable, in dev and in the bundle.
fn sidecar_path(name: &str) -> std::io::Result<PathBuf> {
    let exe = std::env::current_exe()?;
    let dir = exe
        .parent()
        .ok_or_else(|| std::io::Error::other("executable has no folder"))?;
    Ok(dir.join(name))
}

pub fn run() {
    let app = tauri::Builder::default()
        .setup(|app| {
            let run_dir = app.path().app_data_dir()?.join("run");
            let started = tauri::async_runtime::block_on(async {
                let binary = sidecar_path(constants::ARIA2_BINARY).map_err(|e| e.to_string())?;
                Aria2Engine::start(&binary, &run_dir)
                    .await
                    .map_err(|e| e.to_string())
            })
            .map(Arc::new);

            let (engine, health) = match started {
                Ok(engine) => {
                    let version = engine.version().to_owned();
                    (Some(engine), EngineHealth::Ready { version })
                }
                Err(reason) => (None, EngineHealth::Failed { reason }),
            };

            let state = AppState {
                engine,
                health,
                video: Mutex::new(None),
                tools_dir: app.path().app_data_dir()?.join("tools"),
                fetching_tools: AtomicBool::new(false),
                download_dir: app.path().download_dir().ok(),
                jobs: Mutex::new(Vec::new()),
                snapshot: Mutex::new(Vec::new()),
            };
            state.enable_video_if_ready();
            app.manage(state);

            start_browser_integration(app.handle());
            tauri::async_runtime::spawn(poll_jobs(app.handle().clone()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            engine_health,
            list_downloads,
            add_download,
            pause_download,
            resume_download,
            remove_download,
            tools_status,
            fetch_tools,
            add_video,
        ])
        .build(tauri::generate_context!())
        .expect("the app could not be built");

    app.run(|app, event| {
        if let RunEvent::Exit = event {
            if let Some(engine) = app.state::<AppState>().engine.clone() {
                tauri::async_runtime::block_on(engine.shutdown());
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_values_cannot_carry_a_second_header() {
        assert!(header_safe("session=abc; theme=dark"));
        assert!(!header_safe("a=b\r\nX-Injected: 1"));
        assert!(!header_safe("a=b\nc"));
        assert!(!header_safe(""));
        assert!(!header_safe(&"x".repeat(MAX_HEADER_VALUE + 1)));
    }

    #[test]
    fn suggested_names_are_reduced_to_a_plain_file_name() {
        assert_eq!(safe_filename("report.pdf").as_deref(), Some("report.pdf"));
        assert_eq!(
            safe_filename("/Users/me/Downloads/report.pdf").as_deref(),
            Some("report.pdf")
        );
        assert_eq!(
            safe_filename("../../.ssh/authorized_keys").as_deref(),
            Some("authorized_keys")
        );
        assert_eq!(safe_filename("C:\\tmp\\a.zip").as_deref(), Some("a.zip"));
        assert_eq!(safe_filename(".zshrc"), None);
        assert_eq!(safe_filename("dir/"), None);
        assert_eq!(safe_filename(".."), None);
        assert_eq!(safe_filename("a\u{0}b"), None);
    }
}
