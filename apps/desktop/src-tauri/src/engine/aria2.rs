//! aria2 as an [`Engine`]: one `aria2c` child process driven over JSON-RPC.
//!
//! aria2 is GPL software. It runs as a separate executable and none of its code
//! is linked here.

use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use serde_json::{json, Value};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use super::{AddRequest, Engine, EngineError, JobState, JobStatus, MAX_CONNECTIONS};

const START_ATTEMPTS: u32 = 3;
const READY_POLLS: u32 = 50;
const READY_POLL_INTERVAL: Duration = Duration::from_millis(100);
const SHUTDOWN_GRACE: Duration = Duration::from_secs(3);

const STATUS_KEYS: [&str; 7] = [
    "gid",
    "status",
    "totalLength",
    "completedLength",
    "downloadSpeed",
    "connections",
    "errorMessage",
];

// No `Debug`: this holds the RPC secret, which must never reach a log.
pub struct Aria2Engine {
    child: Mutex<Child>,
    http: reqwest::Client,
    endpoint: String,
    secret: String,
    version: String,
}

impl Aria2Engine {
    /// Starts `aria2c` with RPC on a random 127.0.0.1 port and a fresh secret.
    ///
    /// `run_dir` briefly holds a private config file carrying the secret, so the
    /// secret never appears in the process list.
    pub async fn start(binary: &Path, run_dir: &Path) -> Result<Self, EngineError> {
        let mut last = EngineError::Start("aria2c was not started".into());
        // The free port is chosen here and bound by aria2 a moment later; another
        // process can take it in between, so try more than once.
        for _ in 0..START_ATTEMPTS {
            match Self::try_start(binary, run_dir).await {
                Ok(engine) => return Ok(engine),
                Err(error) => last = error,
            }
        }
        Err(last)
    }

    async fn try_start(binary: &Path, run_dir: &Path) -> Result<Self, EngineError> {
        let port = free_local_port()?;
        let secret = random_secret()?;
        let conf = SecretFile::create(run_dir, &format!("rpc-secret={secret}\n"))?;

        let child = Command::new(binary)
            .arg(format!("--conf-path={}", conf.path.display()))
            .arg("--enable-rpc=true")
            .arg("--rpc-listen-all=false")
            .arg(format!("--rpc-listen-port={port}"))
            .arg("--rpc-allow-origin-all=false")
            // aria2 exits by itself if the app dies without cleaning up.
            .arg(format!("--stop-with-process={}", std::process::id()))
            .arg("--no-netrc=true")
            .arg("--quiet=true")
            .arg("--console-log-level=error")
            .arg("--summary-interval=0")
            .arg("--continue=true")
            .arg("--file-allocation=none")
            .arg("--min-split-size=1M")
            .arg(format!("--max-connection-per-server={MAX_CONNECTIONS}"))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| EngineError::Start(format!("could not launch aria2c: {e}")))?;

        let http = reqwest::Client::builder()
            // Never send loopback RPC through a system proxy.
            .no_proxy()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| EngineError::Start(format!("could not build the RPC client: {e}")))?;

        // From here on, dropping `engine` kills the child.
        let mut engine = Self {
            child: Mutex::new(child),
            http,
            endpoint: format!("http://127.0.0.1:{port}/jsonrpc"),
            secret,
            version: String::new(),
        };

        for _ in 0..READY_POLLS {
            if let Ok(Some(status)) = engine.child.lock().await.try_wait() {
                return Err(EngineError::Start(format!("aria2c exited early: {status}")));
            }
            if let Ok(reply) = engine.call("aria2.getVersion", vec![]).await {
                engine.version = reply["version"].as_str().unwrap_or_default().to_owned();
                return Ok(engine);
            }
            tokio::time::sleep(READY_POLL_INTERVAL).await;
        }
        Err(EngineError::Start(
            "aria2c did not answer on its RPC port".into(),
        ))
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    /// Asks aria2 to save its control files and exit, then makes sure it is gone.
    pub async fn shutdown(&self) {
        let _ = self.call("aria2.shutdown", vec![]).await;
        let mut child = self.child.lock().await;
        if tokio::time::timeout(SHUTDOWN_GRACE, child.wait())
            .await
            .is_err()
        {
            let _ = child.kill().await;
        }
    }

    async fn call(&self, method: &str, params: Vec<Value>) -> Result<Value, EngineError> {
        let mut all = Vec::with_capacity(params.len() + 1);
        all.push(Value::String(format!("token:{}", self.secret)));
        all.extend(params);
        let body = json!({ "jsonrpc": "2.0", "id": "mbiro", "method": method, "params": all });

        // Errors are reported without the request, which carries the secret.
        let transport = |e: reqwest::Error| EngineError::Transport(e.without_url().to_string());
        let reply: Value = self
            .http
            .post(&self.endpoint)
            .json(&body)
            .send()
            .await
            .map_err(transport)?
            .json()
            .await
            .map_err(transport)?;

        if let Some(error) = reply.get("error") {
            let message = error["message"]
                .as_str()
                .unwrap_or("aria2 reported an error");
            return Err(EngineError::Rejected(message.to_owned()));
        }
        Ok(reply["result"].clone())
    }
}

impl Engine for Aria2Engine {
    async fn add(&self, request: AddRequest) -> Result<String, EngineError> {
        let connections = request.connections.clamp(1, MAX_CONNECTIONS).to_string();
        let options = json!({
            "dir": request.dir,
            "split": connections,
            "max-connection-per-server": connections,
        });
        let gid = self
            .call("aria2.addUri", vec![json!([request.url.as_str()]), options])
            .await?;
        gid.as_str()
            .map(str::to_owned)
            .ok_or_else(|| EngineError::Rejected("aria2 returned no download id".into()))
    }

    async fn pause(&self, id: &str) -> Result<(), EngineError> {
        self.call("aria2.pause", vec![json!(id)]).await.map(drop)
    }

    async fn resume(&self, id: &str) -> Result<(), EngineError> {
        self.call("aria2.unpause", vec![json!(id)]).await.map(drop)
    }

    async fn remove(&self, id: &str) -> Result<(), EngineError> {
        // A running or waiting download is removed; a finished or failed one only
        // has a result record left to clear.
        match self.call("aria2.remove", vec![json!(id)]).await {
            Ok(_) => Ok(()),
            Err(EngineError::Rejected(_)) => self
                .call("aria2.removeDownloadResult", vec![json!(id)])
                .await
                .map(drop),
            Err(other) => Err(other),
        }
    }

    async fn status(&self, id: &str) -> Result<JobStatus, EngineError> {
        let mut keys: Vec<&str> = STATUS_KEYS.to_vec();
        keys.push("files");
        let reply = self
            .call("aria2.tellStatus", vec![json!(id), json!(keys)])
            .await?;
        Ok(parse_status(&reply))
    }
}

fn parse_status(reply: &Value) -> JobStatus {
    let number = |key: &str| -> u64 {
        reply[key]
            .as_str()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0)
    };
    let state = match reply["status"].as_str().unwrap_or_default() {
        "active" => JobState::Active,
        "waiting" => JobState::Queued,
        "paused" => JobState::Paused,
        "complete" => JobState::Done,
        _ => JobState::Failed,
    };
    let error = (state == JobState::Failed)
        .then(|| reply["errorMessage"].as_str().map(str::to_owned))
        .flatten()
        .filter(|message| !message.is_empty());

    JobStatus {
        id: reply["gid"].as_str().unwrap_or_default().to_owned(),
        state,
        name: file_name(&reply["files"]),
        total_bytes: number("totalLength"),
        completed_bytes: number("completedLength"),
        speed_bps: if state == JobState::Active {
            number("downloadSpeed")
        } else {
            0
        },
        connections: number("connections") as u32,
        error,
    }
}

/// The saved file's name once aria2 knows it, otherwise the last part of the URL.
fn file_name(files: &Value) -> String {
    let first = &files[0];
    let from_path = first["path"]
        .as_str()
        .and_then(|p| Path::new(p).file_name())
        .map(|n| n.to_string_lossy().into_owned());
    let from_uri = || {
        let uri = first["uris"][0]["uri"].as_str()?;
        let parsed = url::Url::parse(uri).ok()?;
        let last = parsed.path_segments()?.next_back()?.to_owned();
        Some(if last.is_empty() {
            parsed.host_str()?.to_owned()
        } else {
            last
        })
    };
    from_path
        .filter(|n| !n.is_empty())
        .or_else(from_uri)
        .unwrap_or_default()
}

fn free_local_port() -> Result<u16, EngineError> {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .and_then(|listener| listener.local_addr())
        .map(|addr| addr.port())
        .map_err(|e| EngineError::Start(format!("no free local port: {e}")))
}

/// 32 bytes from the OS random source, as hex.
fn random_secret() -> Result<String, EngineError> {
    let mut bytes = [0u8; 32];
    std::fs::File::open("/dev/urandom")
        .and_then(|mut source| source.read_exact(&mut bytes))
        .map_err(|e| EngineError::Start(format!("no random source: {e}")))?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}

/// A file only this user can read, removed when dropped.
struct SecretFile {
    path: PathBuf,
}

impl SecretFile {
    fn create(dir: &Path, contents: &str) -> Result<Self, EngineError> {
        use std::os::unix::fs::OpenOptionsExt;

        let failed = |e: std::io::Error| EngineError::Start(format!("could not write config: {e}"));
        std::fs::create_dir_all(dir).map_err(failed)?;
        let path = dir.join(format!("aria2-{}.conf", std::process::id()));
        let _ = std::fs::remove_file(&path);
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&path)
            .map_err(failed)?;
        let guard = Self { path };
        file.write_all(contents.as_bytes()).map_err(failed)?;
        Ok(guard)
    }
}

impl Drop for SecretFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_aria2_status_to_job_status() {
        let reply = json!({
            "gid": "2089b05ecca3d829",
            "status": "active",
            "totalLength": "34896138",
            "completedLength": "8749056",
            "downloadSpeed": "1048576",
            "connections": "8",
            "files": [{ "path": "/Users/me/Downloads/file.zip", "uris": [] }],
        });
        let status = parse_status(&reply);
        assert_eq!(status.id, "2089b05ecca3d829");
        assert_eq!(status.state, JobState::Active);
        assert_eq!(status.name, "file.zip");
        assert_eq!(status.total_bytes, 34_896_138);
        assert_eq!(status.completed_bytes, 8_749_056);
        assert_eq!(status.speed_bps, 1_048_576);
        assert_eq!(status.connections, 8);
        assert!(status.error.is_none());
    }

    #[test]
    fn failed_status_carries_the_reason_and_no_speed() {
        let reply = json!({
            "gid": "1",
            "status": "error",
            "downloadSpeed": "99",
            "errorMessage": "Resource not found",
            "files": [{ "path": "", "uris": [{ "uri": "http://127.0.0.1:1/a/b.bin?x=1" }] }],
        });
        let status = parse_status(&reply);
        assert_eq!(status.state, JobState::Failed);
        assert_eq!(status.name, "b.bin");
        assert_eq!(status.speed_bps, 0);
        assert_eq!(status.error.as_deref(), Some("Resource not found"));
    }

    #[test]
    fn secrets_are_long_and_differ() {
        let (a, b) = (random_secret().unwrap(), random_secret().unwrap());
        assert_eq!(a.len(), 64);
        assert_ne!(a, b);
    }
}
