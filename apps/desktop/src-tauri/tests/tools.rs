//! The checksum gate every fetched tool passes through, against the local test server.

mod support;

use std::path::PathBuf;
use std::time::Duration;

use mbiro_desktop::tools::{download_verified, ToolError, ALL};
use sha2::{Digest, Sha256};
use support::TestServer;

fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("mbiro-tools-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create scratch folder");
    dir
}

fn hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn keeps_a_file_only_when_its_checksum_matches() {
    let server = TestServer::start(300 * 1024, Duration::ZERO).await;
    let dir = scratch("verify");
    let client = reqwest::Client::new();
    let good = hex(&server.body);

    let kept = dir.join("good.zip");
    let seen = std::sync::Mutex::new((0u64, None));
    download_verified(
        &client,
        &server.url("tool.zip"),
        &good,
        &kept,
        |received, total| {
            *seen.lock().unwrap() = (received, total);
        },
    )
    .await
    .expect("a matching file is kept");
    assert_eq!(std::fs::read(&kept).unwrap(), *server.body);
    assert_eq!(*seen.lock().unwrap(), (300 * 1024, Some(300 * 1024)));

    let wrong = hex(b"something else");
    let dropped = dir.join("bad.zip");
    let result = download_verified(
        &client,
        &server.url("tool.zip"),
        &wrong,
        &dropped,
        |_, _| {},
    )
    .await;
    assert!(matches!(result, Err(ToolError::ChecksumMismatch)));
    assert!(
        !dropped.exists(),
        "a file that fails its checksum must not stay on disk"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "multi_thread")]
async fn nothing_counts_as_installed_in_an_empty_folder() {
    let dir = scratch("empty");
    for tool in ALL {
        assert!(
            !tool.is_installed(&dir),
            "{} reported as installed",
            tool.name
        );
        assert!(tool.executable_path(&dir).starts_with(&dir));
    }
    let _ = std::fs::remove_dir_all(&dir);
}
