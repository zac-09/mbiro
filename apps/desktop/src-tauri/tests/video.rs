//! The whole video path with the real tools: fetch yt-dlp, Deno and ffmpeg,
//! then download a clip whose picture and sound are separate streams and check
//! they come out as one file.
//!
//! The clip is made here with ffmpeg and served from this machine, so no real
//! site is contacted. The tools themselves are fetched from the internet
//! (about 150 MB, kept in `target/test-tools`), so this test only runs when
//! asked for:
//!
//!     cargo test -p mbiro-desktop --test video -- --ignored

mod support;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use mbiro_desktop::engine::ytdlp::{ToolPaths, YtDlpEngine};
use mbiro_desktop::engine::{AddRequest, BrowserContext, Engine, JobState};
use mbiro_desktop::tools::{ALL, DENO, FFMPEG, YT_DLP};
use mbiro_desktop::video::{self, Quality};

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mbiro-video-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch folder");
    dir
}

/// Six seconds of test pattern and a tone, as a DASH manifest with one video
/// stream and one audio stream.
fn make_clip(ffmpeg: &Path, dir: &Path) {
    let status = Command::new(ffmpeg)
        .args(["-loglevel", "error", "-f", "lavfi", "-i"])
        .arg("testsrc=duration=6:size=320x240:rate=25")
        .args(["-f", "lavfi", "-i", "sine=frequency=440:duration=6"])
        .args([
            "-c:v", "libx264", "-pix_fmt", "yuv420p", "-g", "50", "-c:a", "aac",
        ])
        .args(["-f", "dash", "-seg_duration", "2"])
        .args(["-adaptation_sets", "id=0,streams=v id=1,streams=a"])
        .arg(dir.join("clip.mpd"))
        .status()
        .expect("run ffmpeg");
    assert!(status.success(), "ffmpeg could not make the test clip");
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "fetches the real tools from the internet"]
async fn fetches_the_tools_then_downloads_and_merges_a_clip() {
    let tools_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/test-tools");
    for tool in ALL {
        tool.install(&tools_dir, |_, _| {})
            .await
            .unwrap_or_else(|e| panic!("{}: {e}", tool.name));
        assert!(tool.is_installed(&tools_dir));
    }
    let paths = ToolPaths {
        yt_dlp: YT_DLP.executable_path(&tools_dir),
        deno: DENO.executable_path(&tools_dir),
        ffmpeg: FFMPEG.executable_path(&tools_dir),
    };

    let media = scratch("media");
    let out = scratch("out");
    make_clip(&paths.ffmpeg, &media);
    let addr = support::serve_folder(media.clone()).await;

    // yt-dlp must see both helpers at the paths it was given.
    let verbose = Command::new(&paths.yt_dlp)
        .args([
            "--ignore-config",
            "--verbose",
            "--simulate",
            "--js-runtimes",
        ])
        .arg(format!("deno:{}", paths.deno.display()))
        .arg("--ffmpeg-location")
        .arg(&paths.ffmpeg)
        .arg(format!("http://{addr}/clip.mpd"))
        .output()
        .expect("run yt-dlp");
    let log = String::from_utf8_lossy(&verbose.stderr);
    assert!(
        log.contains(&format!("JS runtimes: deno-{}", DENO.version)),
        "{log}"
    );
    assert!(log.contains(&format!("ffmpeg {}", FFMPEG.version)), "{log}");

    let engine = YtDlpEngine::new(paths.clone());
    let request = AddRequest {
        url: format!("http://{addr}/clip.mpd").parse().unwrap(),
        dir: out.clone(),
        connections: 1,
        filename: None,
        context: BrowserContext::default(),
        video: None,
    };
    let id = engine.add(request).await.expect("add");

    let deadline = Instant::now() + Duration::from_secs(120);
    let status = loop {
        let status = engine.status(&id).await.expect("status");
        assert_ne!(status.state, JobState::Failed, "failed: {:?}", status.error);
        if status.state == JobState::Done {
            break status;
        }
        assert!(Instant::now() < deadline, "timed out");
        tokio::time::sleep(Duration::from_millis(200)).await;
    };
    assert_eq!(status.name, "clip.mp4");
    assert!(status.completed_bytes > 0, "no progress was ever reported");

    // One file, holding both the picture and the sound, and no leftovers.
    let files: Vec<_> = std::fs::read_dir(&out)
        .unwrap()
        .flatten()
        .map(|e| e.file_name())
        .collect();
    assert_eq!(files, vec![std::ffi::OsString::from("clip.mp4")]);
    let probe = Command::new(&paths.ffmpeg)
        .args(["-hide_banner", "-i"])
        .arg(out.join("clip.mp4"))
        .output()
        .expect("probe the result");
    let info = String::from_utf8_lossy(&probe.stderr);
    assert!(info.contains("Video: h264"), "{info}");
    assert!(info.contains("Audio: aac"), "{info}");

    engine.remove(&id).await.expect("remove");

    // The update check: ask the stable channel for its newest release and
    // install it, verified against the checksum that release publishes.
    let version = mbiro_desktop::tools::update_yt_dlp(
        &tools_dir,
        mbiro_desktop::tools::Channel::Stable,
        |_, _| {},
    )
    .await
    .expect("update yt-dlp");
    let updated = mbiro_desktop::tools::yt_dlp_executable(&tools_dir).expect("an updated yt-dlp");
    let reported = Command::new(&updated)
        .arg("--version")
        .output()
        .expect("run the updated yt-dlp");
    assert_eq!(String::from_utf8_lossy(&reported.stdout).trim(), version);
    assert!(
        version.as_str() >= YT_DLP.version,
        "the newest release is not older than the pinned one"
    );
    engine.set_yt_dlp(updated);

    // The nightly channel comes from a different repository; it must install
    // and run the same way. Kept in its own folder so the two do not mix.
    let nightly_dir =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../target/test-tools-nightly");
    let nightly = mbiro_desktop::tools::update_yt_dlp(
        &nightly_dir,
        mbiro_desktop::tools::Channel::Nightly,
        |_, _| {},
    )
    .await
    .expect("update to nightly");
    let state = mbiro_desktop::tools::yt_dlp_state(&nightly_dir).expect("nightly state");
    assert_eq!(state.version, nightly);
    assert_eq!(state.channel, mbiro_desktop::tools::Channel::Nightly);
    let exe = mbiro_desktop::tools::yt_dlp_executable(&nightly_dir).expect("a nightly yt-dlp");
    let reported = Command::new(&exe)
        .arg("--version")
        .output()
        .expect("run the nightly yt-dlp");
    assert!(reported.status.success());
    assert!(
        String::from_utf8_lossy(&reported.stdout).trim() >= YT_DLP.version,
        "a nightly is not older than the pinned release"
    );

    // The look-up the add dialog does, and a download of the sound alone.
    let page = format!("http://{addr}/clip.mpd").parse().unwrap();
    let found = video::probe(&paths, &page).await.expect("probe");
    let video::Found::Video(info) = found else {
        panic!("a single clip is not a playlist")
    };
    assert_eq!(info.title, "clip");
    assert_eq!(info.presets[0].quality, Quality::Best);
    assert_eq!(info.presets[0].height, Some(240));
    assert!(info
        .presets
        .iter()
        .any(|preset| preset.quality == Quality::Audio));
    assert!(
        info.formats.iter().any(|row| row.has_video)
            && info.formats.iter().any(|row| !row.has_video)
    );

    let sound = scratch("sound");
    let request = AddRequest {
        url: page,
        dir: sound.clone(),
        connections: 1,
        filename: None,
        context: BrowserContext::default(),
        video: Quality::Audio.options(),
    };
    let id = engine.add(request).await.expect("add audio");
    loop {
        let status = engine.status(&id).await.expect("status");
        assert_ne!(status.state, JobState::Failed, "failed: {:?}", status.error);
        if status.state == JobState::Done {
            break;
        }
        assert!(Instant::now() < deadline, "timed out");
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
    let saved: Vec<_> = std::fs::read_dir(&sound)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .collect();
    assert_eq!(saved.len(), 1, "{saved:?}");
    let probe = Command::new(&paths.ffmpeg)
        .args(["-hide_banner", "-i"])
        .arg(&saved[0])
        .output()
        .unwrap();
    let info = String::from_utf8_lossy(&probe.stderr);
    assert!(
        info.contains("Audio: aac") && !info.contains("Video:"),
        "{info}"
    );
    let _ = std::fs::remove_dir_all(&sound);
    let _ = std::fs::remove_dir_all(&media);
    let _ = std::fs::remove_dir_all(&out);
}
