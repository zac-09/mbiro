//! Drives the real `aria2c` built by `scripts/build-aria2.sh` against the local
//! test server. No real site is contacted.

mod support;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use mbiro_desktop::engine::aria2::Aria2Engine;
use mbiro_desktop::engine::{AddRequest, BrowserContext, Engine, JobState, JobStatus, Secret};
use support::TestServer;

const MB: usize = 1024 * 1024;

fn aria2_binary() -> PathBuf {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("binaries/aria2c-universal-apple-darwin");
    assert!(
        path.exists(),
        "aria2c is missing: run scripts/build-aria2.sh first"
    );
    path
}

/// A fresh, empty folder for one test.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mbiro-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch folder");
    dir
}

async fn wait_for(
    engine: &Aria2Engine,
    id: &str,
    what: &str,
    done: impl Fn(&JobStatus) -> bool,
) -> Vec<JobStatus> {
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut seen = Vec::new();
    loop {
        let status = engine.status(id).await.expect("status");
        assert_ne!(
            status.state,
            JobState::Failed,
            "download failed: {:?}",
            status.error
        );
        let finished = done(&status);
        seen.push(status);
        if finished {
            return seen;
        }
        assert!(Instant::now() < deadline, "timed out waiting for {what}");
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn downloads_a_file_in_segments_and_reports_progress() {
    let server = TestServer::start(16 * MB, Duration::from_millis(20)).await;
    let dir = scratch("segments");
    let engine = Aria2Engine::start(&aria2_binary(), &dir.join("run"))
        .await
        .expect("start aria2");
    assert_eq!(engine.version(), "1.37.0");

    let request = AddRequest {
        url: server.url("sample.bin").parse().unwrap(),
        dir: dir.clone(),
        connections: 8,
        filename: None,
        context: BrowserContext::default(),
    };
    let id = engine.add(request).await.expect("add");
    let seen = wait_for(&engine, &id, "completion", |s| s.state == JobState::Done).await;

    let partial = seen
        .iter()
        .filter(|s| s.state == JobState::Active && s.completed_bytes > 0)
        .filter(|s| s.completed_bytes < s.total_bytes)
        .count();
    assert!(
        partial >= 2,
        "expected live progress, saw {partial} partial readings"
    );
    assert!(
        seen.iter().any(|s| s.speed_bps > 0),
        "expected a speed reading"
    );
    assert!(
        seen.iter().any(|s| s.connections > 1),
        "expected several connections"
    );

    let last = seen.last().unwrap();
    assert_eq!(last.name, "sample.bin");
    assert_eq!(last.total_bytes, (16 * MB) as u64);
    assert_eq!(last.completed_bytes, last.total_bytes);
    assert!(
        server.peak_connections() > 1,
        "the server saw one connection only"
    );

    let saved = std::fs::read(dir.join("sample.bin")).expect("read the saved file");
    assert!(
        saved == *server.body,
        "the saved file differs from what was served"
    );

    engine.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn pauses_resumes_and_finishes_intact() {
    let server = TestServer::start(16 * MB, Duration::from_millis(20)).await;
    let dir = scratch("pause");
    let engine = Aria2Engine::start(&aria2_binary(), &dir.join("run"))
        .await
        .expect("start aria2");

    let request = AddRequest {
        url: server.url("paused.bin").parse().unwrap(),
        dir: dir.clone(),
        connections: 4,
        filename: None,
        context: BrowserContext::default(),
    };
    let id = engine.add(request).await.expect("add");
    wait_for(&engine, &id, "first bytes", |s| s.completed_bytes > 0).await;

    engine.pause(&id).await.expect("pause");
    let paused = wait_for(&engine, &id, "pause", |s| s.state == JobState::Paused).await;
    let at_pause = paused.last().unwrap().completed_bytes;
    assert!(
        at_pause < (16 * MB) as u64,
        "finished before it could be paused"
    );
    assert_eq!(paused.last().unwrap().speed_bps, 0);
    assert!(
        dir.join("paused.bin.aria2").exists(),
        "no control file beside the partial download"
    );

    engine.resume(&id).await.expect("resume");
    let seen = wait_for(&engine, &id, "completion", |s| s.state == JobState::Done).await;
    assert!(seen.last().unwrap().completed_bytes >= at_pause);

    let saved = std::fs::read(dir.join("paused.bin")).expect("read the saved file");
    assert!(
        saved == *server.body,
        "the saved file differs from what was served"
    );

    engine
        .remove(&id)
        .await
        .expect("remove a finished download");
    assert!(
        engine.status(&id).await.is_err(),
        "a removed download is forgotten"
    );

    engine.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn secret_file_is_removed_once_the_engine_is_up() {
    let dir = scratch("secret");
    let run_dir = dir.join("run");
    let engine = Aria2Engine::start(&aria2_binary(), &run_dir)
        .await
        .expect("start aria2");

    let leftovers: Vec<_> = std::fs::read_dir(&run_dir).unwrap().flatten().collect();
    assert!(
        leftovers.is_empty(),
        "the config file with the secret was left on disk"
    );

    engine.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn sends_the_browser_context_and_saves_under_the_given_name() {
    let server = TestServer::start(2 * MB, Duration::from_millis(1)).await;
    let dir = scratch("context");
    let engine = Aria2Engine::start(&aria2_binary(), &dir.join("run"))
        .await
        .expect("start aria2");

    let request = AddRequest {
        url: server.url("download?id=42").parse().unwrap(),
        dir: dir.clone(),
        connections: 2,
        filename: Some("invoice.pdf".into()),
        context: BrowserContext {
            referrer: Some("http://127.0.0.1/account".into()),
            user_agent: Some("TestBrowser/1.0".into()),
            cookies: Some(Secret::new("session=abc123; theme=dark")),
        },
    };
    let id = engine.add(request).await.expect("add");
    let seen = wait_for(&engine, &id, "completion", |s| s.state == JobState::Done).await;
    assert_eq!(seen.last().unwrap().name, "invoice.pdf");
    assert!(dir.join("invoice.pdf").exists());

    let heads = server.request_heads();
    assert!(!heads.is_empty());
    for head in &heads {
        let head = head.to_ascii_lowercase();
        assert!(
            head.contains("cookie: session=abc123; theme=dark\r\n"),
            "no cookies in: {head}"
        );
        assert!(
            head.contains("referer: http://127.0.0.1/account\r\n"),
            "no referrer in: {head}"
        );
        assert!(
            head.contains("user-agent: testbrowser/1.0\r\n"),
            "no user agent in: {head}"
        );
    }

    engine.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}
