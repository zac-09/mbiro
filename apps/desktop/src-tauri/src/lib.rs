//! The Rust core. It owns the engine process and the list of jobs; the window
//! only sends commands and renders the events sent from here.

mod browsers;
mod constants;
pub mod engine;
mod ipc;
mod strings;

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State};
use url::Url;

use engine::aria2::Aria2Engine;
use engine::{AddRequest, Engine, JobStatus, DEFAULT_CONNECTIONS};

const POLL_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Clone, Serialize)]
#[serde(tag = "status", rename_all = "lowercase")]
enum EngineHealth {
    Ready { version: String },
    Failed { reason: String },
}

struct AppState {
    engine: Option<Arc<Aria2Engine>>,
    health: EngineHealth,
    download_dir: Option<PathBuf>,
    /// Engine ids of the jobs this run knows about, oldest first.
    jobs: Mutex<Vec<String>>,
    /// What the window was last told.
    snapshot: Mutex<Vec<JobStatus>>,
}

impl AppState {
    fn engine(&self) -> Result<&Arc<Aria2Engine>, String> {
        self.engine
            .as_ref()
            .ok_or_else(|| strings::ENGINE_NOT_RUNNING.to_owned())
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
            Self::NoDownloadFolder => strings::DOWNLOAD_FOLDER_MISSING.to_owned(),
            Self::EngineDown => strings::ENGINE_NOT_RUNNING.to_owned(),
            Self::Engine(reason) => reason,
        }
    }

    /// What the extension is told.
    fn code(&self) -> ipc::RefusalCode {
        match self {
            Self::InvalidUrl | Self::UnsupportedScheme => ipc::RefusalCode::UnsupportedUrl,
            Self::NoDownloadFolder | Self::EngineDown => ipc::RefusalCode::EngineUnavailable,
            Self::Engine(_) => ipc::RefusalCode::Rejected,
        }
    }
}

/// The one way a download starts, whether asked for by the window or a browser.
async fn start_download(state: &AppState, url: &str) -> Result<String, AddError> {
    let url = Url::parse(url.trim()).map_err(|_| AddError::InvalidUrl)?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(AddError::UnsupportedScheme);
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
    };
    let id = engine
        .add(request)
        .await
        .map_err(|e| AddError::Engine(e.to_string()))?;
    state.jobs.lock().expect("jobs lock").push(id.clone());
    Ok(id)
}

#[tauri::command]
async fn add_download(state: State<'_, AppState>, url: String) -> Result<String, String> {
    start_download(&state, &url)
        .await
        .map_err(AddError::message)
}

/// Answers the native messaging host for the life of the app.
async fn serve_browsers(app: AppHandle, listener: tokio::net::UnixListener) {
    ipc::serve(listener, move |request| {
        let app = app.clone();
        async move {
            let ipc::Request::Download { url, .. } = request;
            match start_download(&app.state::<AppState>(), &url).await {
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

#[tauri::command]
async fn pause_download(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.engine()?.pause(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn resume_download(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state.engine()?.resume(&id).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn remove_download(state: State<'_, AppState>, id: String) -> Result<(), String> {
    state
        .engine()?
        .remove(&id)
        .await
        .map_err(|e| e.to_string())?;
    state
        .jobs
        .lock()
        .expect("jobs lock")
        .retain(|job| job != &id);
    Ok(())
}

/// Asks the engine about every job once a second and tells the window.
/// Runs for the life of the app, with or without a window.
async fn poll_jobs(app: AppHandle, engine: Arc<Aria2Engine>) {
    let mut tick = tokio::time::interval(POLL_INTERVAL);
    loop {
        tick.tick().await;
        let state = app.state::<AppState>();
        let ids = state.jobs.lock().expect("jobs lock").clone();
        let mut snapshot = Vec::with_capacity(ids.len());
        for id in &ids {
            if let Ok(status) = engine.status(id).await {
                snapshot.push(status);
            }
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

            app.manage(AppState {
                engine: engine.clone(),
                health,
                download_dir: app.path().download_dir().ok(),
                jobs: Mutex::new(Vec::new()),
                snapshot: Mutex::new(Vec::new()),
            });

            start_browser_integration(app.handle());
            if let Some(engine) = engine {
                tauri::async_runtime::spawn(poll_jobs(app.handle().clone(), engine));
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            engine_health,
            list_downloads,
            add_download,
            pause_download,
            resume_download,
            remove_download,
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
