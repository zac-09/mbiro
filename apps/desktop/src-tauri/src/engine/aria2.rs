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
use tokio::sync::RwLock;

use super::{AddRequest, Engine, EngineError, JobState, JobStatus, MAX_CONNECTIONS};

const START_ATTEMPTS: u32 = 3;
const READY_POLLS: u32 = 50;
const READY_POLL_INTERVAL: Duration = Duration::from_millis(100);
/// aria2 waits about three seconds after a shutdown request before it exits.
const SHUTDOWN_GRACE: Duration = Duration::from_secs(8);

/// aria2 exit codes that mean "the network let us down", not "this link is bad":
/// 2 timed out, 6 network problem, 19 name lookup failed.
const NETWORK_ERROR_CODES: [&str; 3] = ["2", "6", "19"];

const STATUS_KEYS: [&str; 8] = [
    "errorCode",
    "gid",
    "status",
    "totalLength",
    "completedLength",
    "downloadSpeed",
    "connections",
    "errorMessage",
];

/// One running `aria2c` and how to reach it.
struct Process {
    child: Child,
    endpoint: String,
    secret: String,
}

// No `Debug`: this holds the RPC secret, which must never reach a log.
pub struct Aria2Engine {
    binary: PathBuf,
    run_dir: PathBuf,
    http: reqwest::Client,
    process: RwLock<Process>,
    version: String,
}

impl Aria2Engine {
    /// Starts `aria2c` with RPC on a random 127.0.0.1 port and a fresh secret.
    ///
    /// `run_dir` briefly holds a private config file carrying the secret, so the
    /// secret never appears in the process list.
    pub async fn start(binary: &Path, run_dir: &Path) -> Result<Self, EngineError> {
        let http = reqwest::Client::builder()
            // Never send loopback RPC through a system proxy.
            .no_proxy()
            .timeout(Duration::from_secs(10))
            .build()
            .map_err(|e| EngineError::Start(format!("could not build the RPC client: {e}")))?;
        let (process, version) = launch(binary, run_dir, &http).await?;
        Ok(Self {
            binary: binary.to_owned(),
            run_dir: run_dir.to_owned(),
            http,
            process: RwLock::new(process),
            version,
        })
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    /// Starts a new `aria2c` if the current one has died. Returns whether it
    /// did: after a restart aria2 has forgotten every download, and the caller
    /// must add again whatever it still wants.
    pub async fn restart_if_dead(&self) -> Result<bool, EngineError> {
        let mut process = self.process.write().await;
        if matches!(process.child.try_wait(), Ok(None)) {
            return Ok(false);
        }
        let (fresh, _) = launch(&self.binary, &self.run_dir, &self.http).await?;
        *process = fresh;
        Ok(true)
    }

    /// Asks aria2 to save its control files and exit, then makes sure it is gone.
    pub async fn shutdown(&self) {
        // The forced form still saves control files; it only skips slow goodbyes.
        let _ = self.call("aria2.forceShutdown", vec![]).await;
        let mut process = self.process.write().await;
        if tokio::time::timeout(SHUTDOWN_GRACE, process.child.wait())
            .await
            .is_err()
        {
            let _ = process.child.kill().await;
        }
    }

    /// Stops aria2 at once, as a crash would. For tests of recovery.
    #[doc(hidden)]
    pub async fn kill_for_test(&self) {
        let mut process = self.process.write().await;
        let _ = process.child.kill().await;
    }

    /// Caps the total download speed. `None` removes the cap.
    pub async fn set_speed_limit(&self, bytes_per_second: Option<u64>) -> Result<(), EngineError> {
        let limit = bytes_per_second.unwrap_or(0).to_string();
        let options = json!({ "max-overall-download-limit": limit });
        self.call("aria2.changeGlobalOption", vec![options])
            .await
            .map(drop)
    }

    async fn call(&self, method: &str, params: Vec<Value>) -> Result<Value, EngineError> {
        let (endpoint, secret) = {
            let process = self.process.read().await;
            (process.endpoint.clone(), process.secret.clone())
        };
        rpc(&self.http, &endpoint, &secret, method, params).await
    }
}

/// Starts one `aria2c` and waits until it answers.
async fn launch(
    binary: &Path,
    run_dir: &Path,
    http: &reqwest::Client,
) -> Result<(Process, String), EngineError> {
    let mut last = EngineError::Start("aria2c was not started".into());
    // The free port is chosen here and bound by aria2 a moment later; another
    // process can take it in between, so try more than once.
    for _ in 0..START_ATTEMPTS {
        match try_launch(binary, run_dir, http).await {
            Ok(started) => return Ok(started),
            Err(error) => last = error,
        }
    }
    Err(last)
}

async fn try_launch(
    binary: &Path,
    run_dir: &Path,
    http: &reqwest::Client,
) -> Result<(Process, String), EngineError> {
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
        // aria2's defaults are what we want for names: a download with a
        // control file beside it is resumed, and a new download never touches
        // an unrelated file that happens to have the same name (it is saved
        // as "name.1.ext" instead). `--continue` is deliberately left off: it
        // would treat any existing file as a partial copy of the new one.
        .arg("--file-allocation=none")
        .arg("--min-split-size=1M")
        .arg(format!("--max-connection-per-server={MAX_CONNECTIONS}"))
        // The queue lives in the job manager; aria2 runs whatever it is given.
        .arg("--max-concurrent-downloads=64")
        // Ride out a dropped network: keep trying, and save progress often so
        // little is lost if the app is killed.
        .arg("--max-tries=0")
        .arg("--retry-wait=5")
        .arg("--connect-timeout=15")
        .arg("--timeout=30")
        .arg("--max-file-not-found=3")
        .arg("--auto-save-interval=5")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| EngineError::Start(format!("could not launch aria2c: {e}")))?;

    // From here on, dropping `process` kills the child.
    let mut process = Process {
        child,
        endpoint: format!("http://127.0.0.1:{port}/jsonrpc"),
        secret,
    };
    for _ in 0..READY_POLLS {
        if let Ok(Some(status)) = process.child.try_wait() {
            return Err(EngineError::Start(format!("aria2c exited early: {status}")));
        }
        let reply = rpc(
            http,
            &process.endpoint,
            &process.secret,
            "aria2.getVersion",
            vec![],
        )
        .await;
        if let Ok(reply) = reply {
            let version = reply["version"].as_str().unwrap_or_default().to_owned();
            return Ok((process, version));
        }
        tokio::time::sleep(READY_POLL_INTERVAL).await;
    }
    Err(EngineError::Start(
        "aria2c did not answer on its RPC port".into(),
    ))
}

async fn rpc(
    http: &reqwest::Client,
    endpoint: &str,
    secret: &str,
    method: &str,
    params: Vec<Value>,
) -> Result<Value, EngineError> {
    let mut all = Vec::with_capacity(params.len() + 1);
    all.push(Value::String(format!("token:{secret}")));
    all.extend(params);
    let body = json!({ "jsonrpc": "2.0", "id": "mbiro", "method": method, "params": all });

    // Errors are reported without the request, which carries the secret.
    let transport = |e: reqwest::Error| EngineError::Transport(e.without_url().to_string());
    let reply: Value = http
        .post(endpoint)
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

impl Engine for Aria2Engine {
    async fn add(&self, request: AddRequest) -> Result<String, EngineError> {
        let connections = request.connections.clamp(1, MAX_CONNECTIONS).to_string();
        let mut options = json!({
            "dir": request.dir,
            "split": connections,
            "max-connection-per-server": connections,
        });
        if let Some(name) = &request.filename {
            options["out"] = json!(name);
        }
        if let Some(referrer) = &request.context.referrer {
            options["referer"] = json!(referrer);
        }
        if let Some(agent) = &request.context.user_agent {
            options["user-agent"] = json!(agent);
        }
        if let Some(cookies) = &request.context.cookies {
            // Kept in aria2's memory for this download only; no session file is written.
            options["header"] = json!([format!("Cookie: {}", cookies.expose())]);
        }
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
        retryable: state == JobState::Failed
            && NETWORK_ERROR_CODES.contains(&reply["errorCode"].as_str().unwrap_or_default()),
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
