//! The Unix socket the native messaging host connects to.
//!
//! One framed JSON message in, one framed JSON acknowledgement out. Every
//! message is validated here; nothing from the browser is trusted or logged.
//! The TypeScript side of this contract is `packages/protocol`.

use std::future::Future;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use mbiro_common::frame::MAX_FRAME;
use serde::{Deserialize, Serialize};

use crate::engine::Secret;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

const PROTOCOL_VERSION: u32 = 1;
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

/// A message from the extension.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    /// Download this link.
    Download(DownloadRequest),
    /// How are these downloads, which the extension sent earlier, getting on?
    Status { version: u32, ids: Vec<String> },
    /// Bring the app's window forward.
    Open { version: u32 },
    /// The user wants the browser to download this after all: drop it.
    Cancel { version: u32, id: String },
}

impl Request {
    fn version(&self) -> u32 {
        match self {
            Self::Download(request) => request.version,
            Self::Status { version, .. }
            | Self::Open { version }
            | Self::Cancel { version, .. } => *version,
        }
    }
}

/// The most downloads one status request may ask about.
pub const MAX_STATUS_IDS: usize = 50;

#[derive(Debug, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadRequest {
    pub version: u32,
    pub url: String,
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub referrer: Option<String>,
    #[serde(default)]
    pub user_agent: Option<String>,
    /// The `Cookie` header value. Never logged: `Secret` hides it from `Debug`.
    #[serde(default)]
    pub cookies: Option<Secret>,
}

/// Why a request was turned down. The extension owns the wording.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RefusalCode {
    InvalidMessage,
    UnsupportedVersion,
    UnsupportedUrl,
    EngineUnavailable,
    Rejected,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename = "ack")]
pub struct Ack {
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<RefusalCode>,
}

/// One download the extension asked about. Nothing here was not already the
/// extension's own: it sent the download in the first place.
#[derive(Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct JobSummary {
    pub id: String,
    pub name: String,
    pub state: crate::engine::JobState,
    pub total_bytes: u64,
    pub completed_bytes: u64,
    pub finished_at: Option<i64>,
}

#[derive(Debug, PartialEq, Serialize)]
#[serde(tag = "type", rename = "status")]
pub struct StatusReply {
    pub jobs: Vec<JobSummary>,
}

/// Everything the app ever says back.
#[derive(Debug, PartialEq, Serialize)]
#[serde(untagged)]
pub enum Reply {
    Ack(Ack),
    Status(StatusReply),
}

impl From<Ack> for Reply {
    fn from(ack: Ack) -> Self {
        Self::Ack(ack)
    }
}

impl Ack {
    /// Done, with nothing to hand back.
    pub fn done() -> Self {
        Self {
            ok: true,
            id: None,
            code: None,
        }
    }

    pub fn accepted(id: String) -> Self {
        Self {
            ok: true,
            id: Some(id),
            code: None,
        }
    }

    pub fn refused(code: RefusalCode) -> Self {
        Self {
            ok: false,
            id: None,
            code: Some(code),
        }
    }
}

/// Binds the socket, replacing one left by an earlier run. Only this user may connect.
pub fn bind(path: &Path) -> io::Result<UnixListener> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    match std::fs::remove_file(path) {
        Err(e) if e.kind() != io::ErrorKind::NotFound => return Err(e),
        _ => {}
    }
    let listener = UnixListener::bind(path)?;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(listener)
}

/// Answers connections until the listener fails. `handle` decides each request.
pub async fn serve<F, Fut>(listener: UnixListener, handle: F)
where
    F: Fn(Request) -> Fut + Clone + Send + 'static,
    Fut: Future<Output = Reply> + Send,
{
    while let Ok((stream, _)) = listener.accept().await {
        let handle = handle.clone();
        tokio::spawn(async move {
            let _ = tokio::time::timeout(CLIENT_TIMEOUT, answer(stream, handle)).await;
        });
    }
}

async fn answer<F, Fut>(mut stream: UnixStream, handle: F) -> io::Result<()>
where
    F: Fn(Request) -> Fut,
    Fut: Future<Output = Reply>,
{
    let len = stream.read_u32_le().await? as usize;
    if len > MAX_FRAME {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "frame too large",
        ));
    }
    let mut payload = vec![0u8; len];
    stream.read_exact(&mut payload).await?;

    let reply = match serde_json::from_slice::<Request>(&payload) {
        Ok(request) if request.version() != PROTOCOL_VERSION => {
            Ack::refused(RefusalCode::UnsupportedVersion).into()
        }
        Ok(request) => handle(request).await,
        Err(_) => Ack::refused(RefusalCode::InvalidMessage).into(),
    };

    let reply = serde_json::to_vec(&reply).map_err(io::Error::other)?;
    stream.write_u32_le(reply.len() as u32).await?;
    stream.write_all(&reply).await?;
    stream.shutdown().await
}

// The frame length is native-endian on the wire (the browser's rule); every Mac
// this runs on is little-endian, which the explicit `_le` calls above rely on.
#[cfg(not(target_endian = "little"))]
compile_error!("the socket framing assumes a little-endian target");

#[cfg(test)]
mod tests {
    use super::*;
    use mbiro_common::frame::{read_frame, write_frame};
    use std::os::unix::net::UnixStream as StdStream;
    use std::path::PathBuf;

    fn socket(name: &str) -> PathBuf {
        PathBuf::from(format!("/tmp/mbiro-ipc-{}-{name}.sock", std::process::id()))
    }

    /// Sends one frame the way the host does and returns the app's answer.
    async fn ask(path: PathBuf, message: &'static [u8]) -> String {
        tokio::task::spawn_blocking(move || {
            let mut stream = StdStream::connect(path).unwrap();
            write_frame(&mut stream, message).unwrap();
            String::from_utf8(read_frame(&mut stream).unwrap().unwrap()).unwrap()
        })
        .await
        .unwrap()
    }

    async fn start(name: &str) -> PathBuf {
        let path = socket(name);
        let listener = bind(&path).unwrap();
        tokio::spawn(serve(listener, |request| async move {
            match request {
                Request::Download(request) if request.url.starts_with("http") => {
                    Ack::accepted("gid-1".into()).into()
                }
                Request::Download(_) => Ack::refused(RefusalCode::UnsupportedUrl).into(),
                Request::Open { .. } | Request::Cancel { .. } => Ack::done().into(),
                Request::Status { ids, .. } => Reply::Status(StatusReply {
                    jobs: ids
                        .into_iter()
                        .map(|id| JobSummary {
                            id,
                            name: "a.bin".into(),
                            state: crate::engine::JobState::Active,
                            total_bytes: 10,
                            completed_bytes: 4,
                            finished_at: None,
                        })
                        .collect(),
                }),
            }
        }));
        path
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn accepts_a_download_and_returns_its_id() {
        let path = start("ok").await;
        let reply = ask(
            path.clone(),
            br#"{"type":"download","version":1,"url":"http://127.0.0.1/a.bin"}"#,
        )
        .await;
        assert_eq!(reply, r#"{"type":"ack","ok":true,"id":"gid-1"}"#);
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "the socket must be private to the user");
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn refuses_what_it_does_not_understand() {
        let path = start("bad").await;
        let cases: [(&[u8], &str); 4] = [
            (b"not json", "invalid_message"),
            (br#"{"type":"sniff","version":1}"#, "invalid_message"),
            (
                br#"{"type":"download","version":2,"url":"http://x/"}"#,
                "unsupported_version",
            ),
            (
                br#"{"type":"download","version":1,"url":"magnet:?xt=1"}"#,
                "unsupported_url",
            ),
        ];
        for (message, code) in cases {
            let reply = ask(path.clone(), message).await;
            assert_eq!(
                reply,
                format!(r#"{{"type":"ack","ok":false,"code":"{code}"}}"#)
            );
        }
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn reads_the_browser_context_and_keeps_cookies_out_of_debug_output() {
        let message = br#"{"type":"download","version":1,"url":"https://host/a.zip",
            "filename":"a.zip","referrer":"https://host/page","userAgent":"UA/1",
            "cookies":"session=hunter2"}"#;
        let Request::Download(request) = serde_json::from_slice(message).unwrap() else {
            panic!("not a download request")
        };
        assert_eq!(request.filename.as_deref(), Some("a.zip"));
        assert_eq!(request.referrer.as_deref(), Some("https://host/page"));
        assert_eq!(request.user_agent.as_deref(), Some("UA/1"));
        assert_eq!(
            request.cookies.as_ref().unwrap().expose(),
            "session=hunter2"
        );
        assert!(!format!("{request:?}").contains("hunter2"));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn answers_status_open_and_cancel() {
        let path = start("more").await;
        let reply = ask(
            path.clone(),
            br#"{"type":"status","version":1,"ids":["7"]}"#,
        )
        .await;
        assert_eq!(
            reply,
            r#"{"type":"status","jobs":[{"id":"7","name":"a.bin","state":"active","totalBytes":10,"completedBytes":4,"finishedAt":null}]}"#
        );
        let done = r#"{"type":"ack","ok":true}"#;
        assert_eq!(
            ask(path.clone(), br#"{"type":"open","version":1}"#).await,
            done
        );
        assert_eq!(
            ask(path.clone(), br#"{"type":"cancel","version":1,"id":"7"}"#).await,
            done
        );
        let old = ask(path.clone(), br#"{"type":"open","version":9}"#).await;
        assert_eq!(
            old,
            r#"{"type":"ack","ok":false,"code":"unsupported_version"}"#
        );
        let _ = std::fs::remove_file(path);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn binding_again_replaces_a_stale_socket() {
        let path = socket("stale");
        drop(bind(&path).unwrap());
        assert!(bind(&path).is_ok());
        let _ = std::fs::remove_file(path);
    }
}
