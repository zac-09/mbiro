//! A small local HTTP server for engine tests (PLAN.md section 11).
//!
//! It serves one deterministic body at any path, honours `Range`, and throttles
//! each connection so a download lasts long enough to observe. Dropping
//! connections and refusing `Range` are added when M1 needs them.

use std::net::SocketAddr;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};

const CHUNK: usize = 32 * 1024;

pub struct TestServer {
    pub addr: SocketAddr,
    pub body: Arc<Vec<u8>>,
    open: Arc<AtomicUsize>,
    peak: Arc<AtomicUsize>,
}

impl TestServer {
    /// Serves `size` bytes, sending one 32 KB chunk per `chunk_delay` on each connection.
    pub async fn start(size: usize, chunk_delay: Duration) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind test server");
        let addr = listener.local_addr().expect("test server address");
        let body = Arc::new(pattern(size));
        let open = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));

        let (shared, open_c, peak_c) = (body.clone(), open.clone(), peak.clone());
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let (body, open, peak) = (shared.clone(), open_c.clone(), peak_c.clone());
                tokio::spawn(async move {
                    let now = open.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(now, Ordering::SeqCst);
                    let _ = serve(stream, &body, chunk_delay).await;
                    open.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });

        Self {
            addr,
            body,
            open,
            peak,
        }
    }

    pub fn url(&self, name: &str) -> String {
        format!("http://{}/{name}", self.addr)
    }

    /// The most connections that were ever open at once.
    pub fn peak_connections(&self) -> usize {
        self.peak.load(Ordering::SeqCst)
    }

    #[allow(dead_code)]
    pub fn open_connections(&self) -> usize {
        self.open.load(Ordering::SeqCst)
    }
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

async fn serve(mut stream: TcpStream, body: &[u8], delay: Duration) -> std::io::Result<()> {
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
    let is_head = head.starts_with("HEAD ");
    let range = head
        .lines()
        .find_map(|line| {
            line.to_ascii_lowercase()
                .strip_prefix("range: bytes=")
                .map(str::to_owned)
        })
        .and_then(|spec| parse_range(spec.trim(), body.len()));

    let (status, start, end) = match range {
        Some((start, end)) => ("206 Partial Content", start, end),
        None => ("200 OK", 0, body.len().saturating_sub(1)),
    };
    let length = end + 1 - start;
    let mut response = format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/octet-stream\r\nAccept-Ranges: bytes\r\n\
         Content-Length: {length}\r\nConnection: close\r\n"
    );
    if range.is_some() {
        response.push_str(&format!(
            "Content-Range: bytes {start}-{end}/{}\r\n",
            body.len()
        ));
    }
    response.push_str("\r\n");
    stream.write_all(response.as_bytes()).await?;

    if !is_head {
        for chunk in body[start..=end].chunks(CHUNK) {
            stream.write_all(chunk).await?;
            tokio::time::sleep(delay).await;
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
