//! The Rust core. It owns the engines and the job list; the window only sends
//! commands and renders the events sent from here.

mod browsers;
mod constants;
pub mod engine;
mod ipc;
pub mod jobs;
mod strings;
pub mod tools;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager as _, RunEvent, State};
use url::Url;

use engine::aria2::Aria2Engine;
use engine::ytdlp::{ToolPaths, YtDlpEngine};
use engine::{BrowserContext, Secret};
use jobs::{JobView, Kind, Manager, NewJob};

const POLL_INTERVAL: Duration = Duration::from_secs(1);
/// How many downloads run at once until Settings exists.
const DEFAULT_MAX_CONCURRENT: usize = 3;
const NETWORK_RETRY_DELAY: Duration = Duration::from_secs(15);

#[derive(Clone, Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
enum EngineHealth {
    Ready { version: String },
    Failed { reason: String },
}

struct AppState {
    manager: Manager,
    health: EngineHealth,
    tools_dir: PathBuf,
    fetching_tools: AtomicBool,
}

impl AppState {
    fn video_engine_if_ready(&self) -> Option<Arc<YtDlpEngine>> {
        video_engine_if_ready(&self.tools_dir)
    }
}

/// The video engine, if every tool it needs is in place.
fn video_engine_if_ready(root: &std::path::Path) -> Option<Arc<YtDlpEngine>> {
    tools::ALL
        .iter()
        .all(|tool| tool.is_installed(root))
        .then(|| {
            Arc::new(YtDlpEngine::new(ToolPaths {
                yt_dlp: tools::YT_DLP.executable_path(root),
                deno: tools::DENO.executable_path(root),
                ffmpeg: tools::FFMPEG.executable_path(root),
            }))
        })
}

#[tauri::command]
fn engine_health(state: State<'_, AppState>) -> EngineHealth {
    state.health.clone()
}

#[tauri::command]
fn list_downloads(state: State<'_, AppState>) -> Result<Vec<JobView>, String> {
    state.manager.snapshot()
}

/// Why a download could not be started.
enum AddError {
    InvalidUrl,
    UnsupportedScheme,
    InvalidContext,
    EngineDown,
    ToolsMissing,
    BadFolder,
    Storage(String),
}

impl AddError {
    /// What the window shows.
    fn message(self) -> String {
        match self {
            Self::InvalidUrl => strings::URL_INVALID.to_owned(),
            Self::UnsupportedScheme => strings::URL_SCHEME_UNSUPPORTED.to_owned(),
            Self::InvalidContext => strings::REQUEST_INVALID.to_owned(),
            Self::EngineDown => strings::ENGINE_NOT_RUNNING.to_owned(),
            Self::ToolsMissing => strings::TOOLS_MISSING.to_owned(),
            Self::BadFolder => strings::FOLDER_MISSING.to_owned(),
            Self::Storage(reason) => reason,
        }
    }

    /// What the extension is told.
    fn code(&self) -> ipc::RefusalCode {
        match self {
            Self::InvalidUrl | Self::UnsupportedScheme => ipc::RefusalCode::UnsupportedUrl,
            Self::InvalidContext => ipc::RefusalCode::InvalidMessage,
            Self::EngineDown | Self::ToolsMissing => ipc::RefusalCode::EngineUnavailable,
            Self::BadFolder | Self::Storage(_) => ipc::RefusalCode::Rejected,
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
struct DownloadSpec {
    kind: Kind,
    url: String,
    /// A folder chosen in the add dialog. `None` is the default folder.
    dir: Option<String>,
    filename: Option<String>,
    connections: Option<u8>,
    context: BrowserContext,
}

/// The one way a download enters the queue, whether asked for by the window or a browser.
fn queue_download(state: &AppState, spec: DownloadSpec) -> Result<i64, AddError> {
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
    match spec.kind {
        Kind::File if state.manager.file_engine().is_none() => return Err(AddError::EngineDown),
        Kind::Video if !state.manager.has_video_engine() => return Err(AddError::ToolsMissing),
        _ => {}
    }
    let dir = match spec.dir.map(PathBuf::from) {
        Some(dir) if dir.is_absolute() && dir.is_dir() => Some(dir),
        Some(_) => return Err(AddError::BadFolder),
        None => None,
    };
    let job = NewJob {
        kind: spec.kind,
        url,
        dir,
        filename: spec.filename.as_deref().and_then(safe_filename),
        connections: spec.connections,
        context: BrowserContext {
            referrer,
            user_agent,
            cookies,
        },
    };
    state.manager.add(job).map_err(AddError::Storage)
}

#[tauri::command]
fn add_download(
    state: State<'_, AppState>,
    url: String,
    dir: Option<String>,
    connections: Option<u8>,
) -> Result<i64, String> {
    let spec = DownloadSpec {
        kind: Kind::File,
        url,
        dir,
        filename: None,
        connections,
        context: BrowserContext::default(),
    };
    queue_download(&state, spec).map_err(AddError::message)
}

/// Hands a page address to yt-dlp. Which pages work is yt-dlp's business.
#[tauri::command]
fn add_video(state: State<'_, AppState>, url: String, dir: Option<String>) -> Result<i64, String> {
    let spec = DownloadSpec {
        kind: Kind::Video,
        url,
        dir,
        filename: None,
        connections: Some(1),
        context: BrowserContext::default(),
    };
    queue_download(&state, spec).map_err(AddError::message)
}

/// What the add dialog starts from.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AddDefaults {
    dir: PathBuf,
    connections: u8,
    max_connections: u8,
}

#[tauri::command]
fn add_defaults(state: State<'_, AppState>) -> AddDefaults {
    AddDefaults {
        dir: state.manager.default_dir().to_owned(),
        connections: engine::DEFAULT_CONNECTIONS,
        max_connections: engine::MAX_CONNECTIONS,
    }
}

#[tauri::command]
async fn pause_download(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.manager.pause(id).await
}

#[tauri::command]
fn resume_download(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.manager.resume(id)
}

#[tauri::command]
fn retry_download(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.manager.retry(id)
}

#[tauri::command]
async fn remove_download(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.manager.remove(id).await
}

#[tauri::command]
async fn pause_all(state: State<'_, AppState>) -> Result<(), String> {
    state.manager.pause_all().await
}

#[tauri::command]
fn resume_all(state: State<'_, AppState>) -> Result<(), String> {
    state.manager.resume_all()
}

/// Shows a finished download in the Finder.
#[tauri::command]
fn show_file(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    let jobs = state.manager.snapshot()?;
    let job = jobs
        .iter()
        .find(|job| job.id == id)
        .ok_or(strings::FILE_MISSING)?;
    let path = job.path();
    if !path.exists() {
        return Err(strings::FILE_MISSING.to_owned());
    }
    std::process::Command::new("/usr/bin/open")
        .arg("-R")
        .arg(path)
        .status()
        .map(drop)
        .map_err(|_| strings::FILE_MISSING.to_owned())
}

/// Answers the native messaging host for the life of the app.
async fn serve_browsers(app: AppHandle, listener: tokio::net::UnixListener) {
    ipc::serve(listener, move |request| {
        let app = app.clone();
        async move {
            let ipc::Request::Download(request) = request;
            let spec = DownloadSpec {
                kind: Kind::File,
                url: request.url,
                dir: None,
                filename: request.filename,
                connections: None,
                context: BrowserContext {
                    referrer: request.referrer,
                    user_agent: request.user_agent,
                    cookies: request.cookies,
                },
            };
            match queue_download(&app.state::<AppState>(), spec) {
                Ok(id) => ipc::Ack::accepted(id.to_string()),
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
            let progress = ToolProgress {
                name: tool.name,
                received,
                total,
            };
            let _ = app.emit(constants::TOOL_PROGRESS_EVENT, progress);
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
    if let Some(engine) = state.video_engine_if_ready() {
        state.manager.set_video_engine(engine);
    }
    outcome
}

/// Turns the crank once a second and tells the window what the list looks like.
/// Runs for the life of the app, with or without a window.
async fn run_queue(app: AppHandle) {
    let mut tick = tokio::time::interval(POLL_INTERVAL);
    loop {
        tick.tick().await;
        let state = app.state::<AppState>();
        if let Ok(finished) = state.manager.tick().await {
            for job in finished {
                let _ = app.emit(constants::FINISHED_EVENT, job);
            }
        }
        if let Ok(snapshot) = state.manager.snapshot() {
            let _ = app.emit(constants::SNAPSHOT_EVENT, snapshot);
        }
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
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let data_dir = app.path().app_data_dir()?;
            let started = tauri::async_runtime::block_on(async {
                let binary = sidecar_path(constants::ARIA2_BINARY).map_err(|e| e.to_string())?;
                Aria2Engine::start(&binary, &data_dir.join("run"))
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

            let tools_dir = data_dir.join("tools");
            let config = jobs::Config {
                db_path: data_dir.join("jobs.sqlite"),
                default_dir: app.path().download_dir()?,
                max_concurrent: DEFAULT_MAX_CONCURRENT,
                retry_delay: NETWORK_RETRY_DELAY,
            };
            let manager = Manager::open(config, engine, video_engine_if_ready(&tools_dir))?;
            app.manage(AppState {
                manager,
                health,
                tools_dir,
                fetching_tools: AtomicBool::new(false),
            });

            start_browser_integration(app.handle());
            tauri::async_runtime::spawn(run_queue(app.handle().clone()));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            engine_health,
            list_downloads,
            add_defaults,
            add_download,
            add_video,
            pause_download,
            resume_download,
            retry_download,
            remove_download,
            pause_all,
            resume_all,
            show_file,
            tools_status,
            fetch_tools,
        ])
        .build(tauri::generate_context!())
        .expect("the app could not be built");

    app.run(|app, event| {
        if let RunEvent::Exit = event {
            // aria2 saves its control files on the way out, so partial
            // downloads resume after the next launch.
            if let Some(engine) = app.state::<AppState>().manager.file_engine().cloned() {
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
