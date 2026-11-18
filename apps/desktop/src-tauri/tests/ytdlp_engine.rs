//! The video engine's handling of its helper process, with a stand-in for
//! yt-dlp: a script that prints the same lines, slowly, and remembers how far
//! it got. No network and no real tools are involved.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use mbiro_desktop::engine::ytdlp::{ToolPaths, YtDlpEngine};
use mbiro_desktop::engine::{
    AddRequest, BrowserContext, Engine, JobState, JobStatus, VideoOptions,
};

/// Counts to 30, a tenth of a second per step, resuming from `state` if it exists.
/// Records its arguments and its process id so the test can inspect both.
const STAND_IN: &str = r#"#!/bin/bash
here="$(dirname "$0")"
echo "$$" > "$here/pid"
printf '%s\n' "$@" > "$here/args"
start=0
[ -f "$here/state" ] && start="$(cat "$here/state")"
echo "NAME A clip"
for ((i = start + 1; i <= 30; i++)); do
  echo "$i" > "$here/state"
  echo "PROGRESS $((i * 1000)) 30000 NA 10000"
  sleep 0.1
done
echo "DONE $here/A clip.mp4"
"#;

fn stand_in(name: &str) -> (PathBuf, ToolPaths) {
    let dir = std::env::temp_dir().join(format!("mbiro-ytdlp-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("yt-dlp");
    std::fs::write(&script, STAND_IN).unwrap();
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    let tools = ToolPaths {
        yt_dlp: script,
        deno: dir.join("deno"),
        ffmpeg: dir.join("ffmpeg"),
    };
    (dir, tools)
}

fn request(dir: &Path, video: Option<VideoOptions>) -> AddRequest {
    AddRequest {
        url: "http://127.0.0.1/watch".parse().unwrap(),
        dir: dir.to_owned(),
        connections: 1,
        filename: None,
        context: BrowserContext::default(),
        video,
    }
}

async fn wait_for(engine: &YtDlpEngine, id: &str, done: impl Fn(&JobStatus) -> bool) -> JobStatus {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let status = engine.status(id).await.expect("status");
        if done(&status) {
            return status;
        }
        assert!(Instant::now() < deadline, "timed out: {status:?}");
        tokio::time::sleep(Duration::from_millis(30)).await;
    }
}

fn alive(dir: &Path) -> bool {
    let pid = std::fs::read_to_string(dir.join("pid")).unwrap();
    std::process::Command::new("/bin/kill")
        .args(["-0", pid.trim()])
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap()
        .success()
}

#[tokio::test(flavor = "multi_thread")]
async fn pausing_stops_the_process_and_resuming_carries_on() {
    let (dir, tools) = stand_in("pause");
    let engine = YtDlpEngine::new(tools);
    let id = engine.add(request(&dir, None)).await.expect("add");

    let running = wait_for(&engine, &id, |s| s.completed_bytes >= 5000).await;
    assert_eq!(running.state, JobState::Active);
    assert_eq!(running.name, "A clip");
    assert_eq!(running.total_bytes, 30_000);
    assert!(alive(&dir));

    engine.pause(&id).await.expect("pause");
    let paused = engine.status(&id).await.unwrap();
    assert_eq!(paused.state, JobState::Paused);
    assert_eq!(paused.speed_bps, 0);
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!alive(&dir), "pausing must stop the helper process");
    let at_pause: u64 = std::fs::read_to_string(dir.join("state"))
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert_eq!(
        engine.status(&id).await.unwrap().state,
        JobState::Paused,
        "a pause is not a failure"
    );

    engine.resume(&id).await.expect("resume");
    assert_eq!(engine.status(&id).await.unwrap().state, JobState::Active);
    let done = wait_for(&engine, &id, |s| s.state == JobState::Done).await;
    assert_eq!(done.name, "A clip.mp4");
    assert_eq!(done.completed_bytes, 30_000);
    assert!(at_pause < 30, "it was paused before the end");

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn passes_the_chosen_quality_and_keeps_the_link_out_of_the_options() {
    let (dir, tools) = stand_in("args");
    let engine = YtDlpEngine::new(tools);
    let video = VideoOptions {
        format: "ba[ext=m4a]/ba".into(),
        audio_only: true,
    };
    let id = engine.add(request(&dir, Some(video))).await.expect("add");
    wait_for(&engine, &id, |s| s.completed_bytes > 0).await;

    let args = std::fs::read_to_string(dir.join("args")).unwrap();
    let args: Vec<&str> = args.lines().collect();
    let after = |flag: &str| args[args.iter().position(|a| *a == flag).unwrap() + 1];
    assert_eq!(after("--format"), "ba[ext=m4a]/ba");
    assert!(args.contains(&"--extract-audio"));
    assert!(!args.contains(&"--merge-output-format"));
    assert!(args.contains(&"--no-playlist"));
    assert!(args.contains(&"--ignore-config"));
    // The link comes last, after "--", so it can never be read as an option.
    assert_eq!(&args[args.len() - 2..], ["--", "http://127.0.0.1/watch"]);

    engine.remove(&id).await.expect("remove");
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(!alive(&dir), "removing must stop the helper process");
    assert!(engine.status(&id).await.is_err());

    let _ = std::fs::remove_dir_all(&dir);
}
