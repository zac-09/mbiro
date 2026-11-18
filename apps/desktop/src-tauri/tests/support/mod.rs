//! A small local HTTP server for engine tests (PLAN.md section 11).
//!
//! It serves one deterministic body at any path, honours `Range`, and throttles
//! each connection so a download lasts long enough to observe. On request it
//! drops connections mid-file, refuses `Range`, or answers 404.

// Each test file uses its own part of this module.
#![allow(dead_code)]

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const CHUNK: usize = 32 * 1024;

/// How the server should misbehave.
#[derive(Clone, Default)]
pub struct Faults {
    /// Cut this many of the first connections short...
    pub drop_first: usize,
    /// ...after sending this many bytes of the body on each.
    pub drop_after: usize,
    /// Ignore `Range` and always send the whole file.
    pub refuse_range: bool,
    /// Answer every request with 404.
    pub not_found: bool,
}

pub struct TestServer {
    pub addr: SocketAddr,
    pub body: Arc<Vec<u8>>,
    open: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
    sent: Arc<AtomicUsize>,
    heads: Arc<Mutex<Vec<String>>>,
}

impl TestServer {
    /// Serves `size` bytes, sending one 32 KB chunk per `chunk_delay` on each connection.
    pub async fn start(size: usize, chunk_delay: Duration) -> Self {
        Self::start_with(size, chunk_delay, Faults::default()).await
    }

    pub async fn start_with(size: usize, chunk_delay: Duration, faults: Faults) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let addr = listener.local_addr().expect("test server address");
        let server = Self {
            addr,
            body: Arc::new(pattern(size)),
            open: Arc::default(),
            peak: Arc::default(),
            sent: Arc::default(),
            heads: Arc::default(),
        };

        let (body, open, peak) = (
            server.body.clone(),
            server.open.clone(),
            server.peak.clone(),
        );
        let (sent, heads) = (server.sent.clone(), server.heads.clone());
        tokio::spawn(async move {
            let mut accepted = 0usize;
            while let Ok((stream, _)) = listener.accept().await {
                accepted += 1;
                let cut = (accepted <= faults.drop_first).then_some(faults.drop_after);
                let (body, open, peak) = (body.clone(), open.clone(), peak.clone());
                let (sent, heads, faults) = (sent.clone(), heads.clone(), faults.clone());
                tokio::spawn(async move {
                    let now = open.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    let connection = Connection {
                        delay: chunk_delay,
                        cut,
                        faults,
                        sent,
                        heads,
                    };
                    let _ = serve(stream, &body, connection).await;
                    open.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });
        server
    }

    pub fn url(&self, name: &str) -> String {
        format!("http://{}/{name}", self.addr)
    }

    /// The most connections that were ever open at once.
    pub fn peak_connections(&self) -> usize {
        self.peak.load(Ordering::SeqCst)
    }

    /// Body bytes sent so far, over all connections.
    pub fn bytes_sent(&self) -> usize {
        self.sent.load(Ordering::SeqCst)
    }

    /// The request line and headers of every request received so far.
    pub fn request_heads(&self) -> Vec<String> {
        self.heads.lock().unwrap().clone()
    }

    pub fn open_connections(&self) -> usize {
        self.open.load(Ordering::SeqCst)
    }
}

struct Connection {
    delay: Duration,
    /// Stop without warning after this many body bytes.
    cut: Option<usize>,
    faults: Faults,
    sent: Arc<AtomicUsize>,
    heads: Arc<Mutex<Vec<String>>>,
}

/// Bytes that differ along the whole body, so a misplaced segment is caught.
fn pattern(size: usize) -> Vec<u8> {
    let mut state: u32 = 0x9E37_79B9;
    (0..size)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (state >> 24) as u8
        })
        .collect()
}

async fn serve(mut stream: TcpStream, body: &[u8], connection: Connection) -> std::io::Result<()> {
    let mut head = Vec::new();
    let mut buf = [0u8; 1024];
    while !head.windows(4).any(|w| w == b"\r\n\r\n") {
        let n = stream.read(&mut buf).await?;
        if n == 0 || head.len() > 16 * 1024 {
            return Ok(());
        }
        head.extend_from_slice(&buf[..n]);
    }
    let head = String::from_utf8_lossy(&head);
    connection.heads.lock().unwrap().push(head.to_string());
    if connection.faults.not_found {
        let reply = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        stream.write_all(reply.as_bytes()).await?;
        return stream.shutdown().await;
    }
    let is_head = head.starts_with("HEAD ");
    let range = head
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("range: bytes=")
                .map(str::to_owned)
        })
        .and_then(|spec| parse_range(spec.trim(), body.len()))
        .filter(|_| !connection.faults.refuse_range);

    let (status, start, end) = match range {
        Some((start, end)) => ("206 Partial Content", start, end),
        None => ("200 OK", 0, body.len().saturating_sub(1)),
    };
    let length = end + 1 - start;
    let mut response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/octet-stream\r\n\
         Content-Length: {length}\r\nConnection: close\r\n"
    );
    if !connection.faults.refuse_range {
        response.push_str("Accept-Ranges: bytes\r\n");
    }
    if range.is_some() {
        response.push_str(&format!(
            "Content-Range: bytes {start}-{end}/{}\r\n",
            body.len()
        ));
    }
    response.push_str("\r\n");
    stream.write_all(response.as_bytes()).await?;

    if !is_head {
        let mut written = 0usize;
        for chunk in body[start..=end].chunks(CHUNK) {
            if connection.cut.is_some_and(|limit| written >= limit) {
                return Ok(()); // dropped mid-file, as a failing network would
            }
            stream.write_all(chunk).await?;
            written += chunk.len();
            connection.sent.fetch_add(chunk.len(), Ordering::SeqCst);
            tokio::time::sleep(connection.delay).await;
        }
    }
    stream.shutdown().await
}

/// `a-b` or `a-`, clamped to the body. Anything else is treated as no range.
fn parse_range(spec: &str, len: usize) -> Option<(usize, usize)> {
    let (start, end) = spec.split_once('-')?;
    let start: usize = start.parse().ok()?;
    let end = if end.is_empty() {
        len - 1
    } else {
        end.parse::<usize>().ok()?.min(len - 1)
    };
    (start <= end).then_some((start, end))
}

/// Serves the files of one folder, whole, at their names. Enough for a media
/// manifest and its segments.
pub async fn serve_folder(dir: std::path::PathBuf) -> SocketAddr {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind folder server");
    let addr = listener.local_addr().expect("folder server address");
    tokio::spawn(async move {
        while let Ok((mut stream, _)) = listener.accept().await {
            let dir = dir.clone();
            tokio::spawn(async move {
                let mut head = Vec::new();
                let mut buf = [0u8; 1024];
                while !head.windows(4).any(|w| w == b"\r\n\r\n") {
                    match stream.read(&mut buf).await {
                        Ok(n) if n > 0 && head.len() < 16 * 1024 => {
                            head.extend_from_slice(&buf[..n])
                        }
                        _ => return,
                    }
                }
                let head = String::from_utf8_lossy(&head).into_owned();
                let name = head
                    .split_whitespace()
                    .nth(1)
                    .and_then(|path| path.split('?').next())
                    .map(|path| path.trim_start_matches('/').to_owned())
                    .filter(|name| !name.is_empty() && !name.contains(['/', '\\']) && name != "..");
                let file = match name {
                    Some(name) => tokio::fs::read(dir.join(name)).await.ok(),
                    None => None,
                };
                let response = match &file {
                    Some(bytes) => format!(
                        "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                        bytes.len()
                    ),
                    None => {
                        "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                            .into()
                    }
                };
                let _ = stream.write_all(response.as_bytes()).await;
                if let Some(bytes) = file {
                    if !head.starts_with("HEAD ") {
                        let _ = stream.write_all(&bytes).await;
                    }
                }
                let _ = stream.shutdown().await;
            });
        }
    });
    addr
}
