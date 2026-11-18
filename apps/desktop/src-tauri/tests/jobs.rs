//! The job manager with the real `aria2c`, against the local test server:
//! the queue, quitting and relaunching, a failing network, and a dead engine.

mod support;

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use mbiro_desktop::engine::aria2::Aria2Engine;
use mbiro_desktop::engine::{BrowserContext, JobState, Secret};
use mbiro_desktop::jobs::{Config, JobView, Kind, Manager, NewJob};
use support::{Faults, TestServer};

const MB: usize = 1024 * 1024;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mbiro-jobs-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch folder");
    dir
}

async fn engine(dir: &Path) -> Arc<Aria2Engine> {
    let binary =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("binaries/aria2c-universal-apple-darwin");
    assert!(
        binary.exists(),
        "aria2c is missing: run scripts/build-aria2.sh first"
    );
    Arc::new(
        Aria2Engine::start(&binary, &dir.join("run"))
            .await
            .expect("start aria2"),
    )
}

fn manager(dir: &Path, engine: Arc<Aria2Engine>, max_concurrent: usize) -> Manager {
    let config = Config {
        db_path: dir.join("jobs.sqlite"),
        default_dir: dir.join("downloads"),
        max_concurrent,
        retry_delay: Duration::from_millis(300),
    };
    Manager::open(config, Some(engine), None).expect("open the job list")
}

fn file(url: String, connections: u8) -> NewJob {
    NewJob {
        kind: Kind::File,
        url: url.parse().unwrap(),
        dir: None,
        filename: None,
        connections: Some(connections),
        context: BrowserContext::default(),
        video: None,
        title: None,
    }
}

/// Ticks ten times a second until `done` says so, and returns every snapshot seen.
async fn run_until(
    manager: &Manager,
    what: &str,
    done: impl Fn(&[JobView]) -> bool,
) -> Vec<Vec<JobView>> {
    let deadline = Instant::now() + Duration::from_secs(90);
    let mut seen = Vec::new();
    loop {
        manager.tick().await.expect("tick");
        let snapshot = manager.snapshot().expect("snapshot");
        let finished = done(&snapshot);
        seen.push(snapshot);
        if finished {
            return seen;
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for {what}: {:?}",
            seen.last()
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

fn all_done(jobs: &[JobView]) -> bool {
    jobs.iter().all(|job| job.state == JobState::Done)
}

fn cookies_in_db(dir: &Path, id: i64) -> Option<String> {
    let db = rusqlite::Connection::open(dir.join("jobs.sqlite")).unwrap();
    db.query_row("SELECT cookies FROM jobs WHERE id = ?1", [id], |row| {
        row.get(0)
    })
    .unwrap()
}

#[tokio::test(flavor = "multi_thread")]
async fn runs_no_more_downloads_at_once_than_allowed() {
    let server = TestServer::start(2 * MB, Duration::from_millis(10)).await;
    let dir = scratch("queue");
    let engine = engine(&dir).await;
    let manager = manager(&dir, engine.clone(), 1);

    let first = manager.add(file(server.url("one.bin"), 2)).unwrap();
    let second = manager.add(file(server.url("two.bin"), 2)).unwrap();
    let seen = run_until(&manager, "both to finish", all_done).await;

    for snapshot in &seen {
        let running = snapshot
            .iter()
            .filter(|job| job.state == JobState::Active)
            .count();
        assert!(running <= 1, "two downloads ran at once");
    }
    assert!(seen
        .iter()
        .any(|s| s[0].state == JobState::Active && s[1].state == JobState::Queued));
    let last = seen.last().unwrap();
    assert_eq!((last[0].id, last[1].id), (first, second));
    for job in last {
        assert!(job.finished_at.is_some());
        assert!(std::fs::read(job.path()).unwrap() == *server.body);
    }

    engine.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn resumes_after_quit_and_relaunch_without_starting_over() {
    // Slow enough that the download is still far from done when aria2, which
    // keeps working for a few seconds after being asked to stop, finally exits.
    let server = TestServer::start(24 * MB, Duration::from_millis(100)).await;
    let dir = scratch("relaunch");

    // First run: get part of the way, then quit.
    let first_engine = engine(&dir).await;
    let first = manager(&dir, first_engine.clone(), 3);
    let mut job = file(server.url("big.bin"), 8);
    job.context.cookies = Some(Secret::new("session=abc123"));
    let id = first.add(job).unwrap();
    run_until(&first, "a few megabytes", |jobs| {
        jobs[0].completed_bytes > 2 * MB as u64
    })
    .await;
    assert_eq!(cookies_in_db(&dir, id).as_deref(), Some("session=abc123"));
    first_engine.shutdown().await;
    drop(first);
    let sent_before = server.bytes_sent();
    let left: Vec<_> = std::fs::read_dir(dir.join("downloads"))
        .map(|entries| entries.flatten().map(|e| e.file_name()).collect())
        .unwrap_or_default();
    assert!(
        dir.join("downloads/big.bin.aria2").exists(),
        "no control file was left: {left:?}"
    );

    // Second run: a new aria2 and a new manager on the same job list.
    let second_engine = engine(&dir).await;
    let second = manager(&dir, second_engine.clone(), 3);
    let restored = second.snapshot().unwrap();
    assert_eq!(restored.len(), 1);
    assert_eq!(
        restored[0].state,
        JobState::Queued,
        "a running job goes back in the queue"
    );
    assert!(restored[0].completed_bytes > 0, "progress is remembered");
    let seen = run_until(&second, "completion", all_done).await;

    assert!(
        seen.iter().flatten().all(|job| !job.resume_doubtful),
        "a normal download is not flagged"
    );
    let job = &seen.last().unwrap()[0];
    assert_eq!(job.name, "big.bin");
    assert!(
        std::fs::read(job.path()).unwrap() == *server.body,
        "the file is not intact"
    );
    let sent_after = server.bytes_sent() - sent_before;
    assert!(
        sent_after < 23 * MB,
        "the second run fetched {sent_after} bytes: it started over instead of resuming"
    );
    assert_eq!(
        cookies_in_db(&dir, id),
        None,
        "cookies are deleted when the job finishes"
    );
    assert!(server
        .request_heads()
        .iter()
        .all(|h| h.contains("session=abc123")));

    second_engine.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn finishes_intact_when_connections_are_dropped() {
    let faults = Faults {
        drop_first: 6,
        drop_after: 300 * 1024,
        ..Faults::default()
    };
    let server = TestServer::start_with(8 * MB, Duration::from_millis(2), faults).await;
    let dir = scratch("drops");
    let engine = engine(&dir).await;
    let manager = manager(&dir, engine.clone(), 3);

    manager.add(file(server.url("flaky.bin"), 4)).unwrap();
    let seen = run_until(&manager, "completion despite drops", all_done).await;
    let job = &seen.last().unwrap()[0];
    assert!(
        std::fs::read(job.path()).unwrap() == *server.body,
        "the file is not intact"
    );

    engine.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn carries_on_when_aria2_dies() {
    let server = TestServer::start(12 * MB, Duration::from_millis(20)).await;
    let dir = scratch("crash");
    let engine = engine(&dir).await;
    let manager = manager(&dir, engine.clone(), 3);

    manager.add(file(server.url("crash.bin"), 4)).unwrap();
    run_until(&manager, "some progress", |jobs| {
        jobs[0].completed_bytes > 2 * MB as u64
    })
    .await;
    engine.kill_for_test().await;

    let seen = run_until(&manager, "completion after the crash", all_done).await;
    let job = &seen.last().unwrap()[0];
    assert!(
        std::fs::read(job.path()).unwrap() == *server.body,
        "the file is not intact"
    );

    engine.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn downloads_on_one_connection_when_the_server_refuses_ranges() {
    let faults = Faults {
        refuse_range: true,
        ..Faults::default()
    };
    let server = TestServer::start_with(8 * MB, Duration::from_millis(2), faults).await;
    let dir = scratch("norange");
    let engine = engine(&dir).await;
    let manager = manager(&dir, engine.clone(), 3);

    manager.add(file(server.url("whole.bin"), 8)).unwrap();
    let seen = run_until(&manager, "completion", all_done).await;
    let job = &seen.last().unwrap()[0];
    assert!(
        std::fs::read(job.path()).unwrap() == *server.body,
        "the file is not intact"
    );
    assert!(seen.iter().flatten().all(|job| job.connections <= 1));
    assert!(
        seen.iter().flatten().any(|job| job.resume_doubtful),
        "a download the server will not resume should be flagged"
    );

    engine.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn pause_frees_the_slot_and_a_missing_file_fails_then_can_be_retried() {
    let server = TestServer::start(8 * MB, Duration::from_millis(20)).await;
    let missing = TestServer::start_with(
        MB,
        Duration::ZERO,
        Faults {
            not_found: true,
            ..Faults::default()
        },
    )
    .await;
    let dir = scratch("pause");
    let engine = engine(&dir).await;
    let manager = manager(&dir, engine.clone(), 1);

    let slow = manager.add(file(server.url("slow.bin"), 2)).unwrap();
    let gone = manager.add(file(missing.url("gone.bin"), 2)).unwrap();
    run_until(&manager, "the first to start", |jobs| {
        jobs[0].completed_bytes > 0
    })
    .await;

    // Pausing the running job lets the next one take the slot; it then fails.
    manager.pause(slow).await.unwrap();
    let seen = run_until(&manager, "the missing file to fail", |jobs| {
        jobs[1].state == JobState::Failed
    })
    .await;
    let last = seen.last().unwrap();
    assert_eq!(last[0].state, JobState::Paused);
    assert_eq!(last[0].speed_bps, 0);
    assert_eq!(
        last[1].error.as_deref(),
        Some("The file was not found on the server. Check the link."),
        "a failure is explained in a plain sentence"
    );

    manager.retry(gone).unwrap();
    assert_eq!(manager.snapshot().unwrap()[1].state, JobState::Queued);
    manager.remove(gone).await.unwrap();

    manager.resume(slow).unwrap();
    let seen = run_until(&manager, "the paused job to finish", all_done).await;
    let job = &seen.last().unwrap()[0];
    assert!(
        std::fs::read(job.path()).unwrap() == *server.body,
        "the file is not intact"
    );
    assert_eq!(
        seen.last().unwrap().len(),
        1,
        "a removed job is gone from the list"
    );

    engine.shutdown().await;
    let _ = std::fs::remove_dir_all(&dir);
}
