//! The one trait every download engine sits behind (PLAN.md section 4).
//!
//! The job manager and the UI see only this module. aria2 is one implementation;
//! yt-dlp will be another.

pub mod aria2;

use std::fmt;
use std::future::Future;
use std::path::PathBuf;

use serde::Serialize;
use url::Url;

/// Segmentation defaults and limits from PLAN.md section 4.
pub const DEFAULT_CONNECTIONS: u8 = 8;
pub const MAX_CONNECTIONS: u8 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum JobState {
    Queued,
    Active,
    Paused,
    Done,
    Failed,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobStatus {
    pub id: String,
    pub state: JobState,
    pub name: String,
    pub total_bytes: u64,
    pub completed_bytes: u64,
    pub speed_bps: u64,
    pub connections: u32,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AddRequest {
    pub url: Url,
    pub dir: PathBuf,
    pub connections: u8,
}

#[derive(Debug)]
pub enum EngineError {
    /// The helper process could not be started or did not come up.
    Start(String),
    /// The helper could not be reached.
    Transport(String),
    /// The helper answered with an error.
    Rejected(String),
}

impl fmt::Display for EngineError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Start(m) | Self::Transport(m) | Self::Rejected(m) => f.write_str(m),
        }
    }
}

impl std::error::Error for EngineError {}

pub trait Engine: Send + Sync {
    /// Starts a download and returns the engine's id for it.
    fn add(&self, request: AddRequest) -> impl Future<Output = Result<String, EngineError>> + Send;
    fn pause(&self, id: &str) -> impl Future<Output = Result<(), EngineError>> + Send;
    fn resume(&self, id: &str) -> impl Future<Output = Result<(), EngineError>> + Send;
    /// Stops the download and forgets it. Files on disk are left alone.
    fn remove(&self, id: &str) -> impl Future<Output = Result<(), EngineError>> + Send;
    fn status(&self, id: &str) -> impl Future<Output = Result<JobStatus, EngineError>> + Send;
}
