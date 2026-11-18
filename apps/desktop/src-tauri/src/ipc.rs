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
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};

const PROTOCOL_VERSION: u32 = 1;
const CLIENT_TIMEOUT: Duration = Duration::from_secs(10);

/// A message from the extension.
#[derive(Debug, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Request {
    Download { version: u32, url: String },
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

impl Ack {
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
    Fut: Future<Output = Ack> + Send,
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
    Fut: Future<Output = Ack>,
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

    let ack = match serde_json::from_slice::<Request>(&payload) {
        Ok(Request::Download { version, .. }) if version != PROTOCOL_VERSION => {
            Ack::refused(RefusalCode::UnsupportedVersion)
        }
        Ok(request) => handle(request).await,
        Err(_) => Ack::refused(RefusalCode::InvalidMessage),
    };

    let reply = serde_json::to_vec(&ack).map_err(io::Error::other)?;
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
                Request::Download { url, .. } if url.starts_with("http") => {
                    Ack::accepted("gid-1".into())
                }
                Request::Download { .. } => Ack::refused(RefusalCode::UnsupportedUrl),
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

    #[tokio::test(flavor = "multi_thread")]
    async fn binding_again_replaces_a_stale_socket() {
        let path = socket("stale");
        drop(bind(&path).unwrap());
        assert!(bind(&path).is_ok());
        let _ = std::fs::remove_file(path);
    }
}
