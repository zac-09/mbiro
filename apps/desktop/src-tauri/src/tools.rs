//! The helper tools the video grabber needs: yt-dlp, Deno and ffmpeg.
//!
//! None of them ships in the app bundle. Each is fetched on demand over HTTPS
//! into Application Support and checked against a pinned SHA-256 before it is
//! unpacked, so nothing unverified is ever executed.
//!
//! Spike 3 pins exact versions here. The daily yt-dlp update, driven by the
//! checksum file each release publishes, comes in M3.

use std::fmt;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;

pub struct Archive {
    pub url: &'static str,
    pub sha256: &'static str,
}

pub struct Tool {
    pub name: &'static str,
    pub version: &'static str,
    arm64: Archive,
    x86_64: Archive,
    /// The executable's path inside the unpacked zip.
    executable: &'static str,
}

/// Official macOS build, both architectures in one file. The folder build is
/// used, not the single-file one: the single file unpacks itself on every
/// start, which costs about ten seconds each time.
/// Checksum from the release's `SHA2-256SUMS`.
pub const YT_DLP: Tool = Tool {
    name: "yt-dlp",
    version: "2026.10.26",
    arm64: Archive {
        url: "https://github.com/yt-dlp/yt-dlp/releases/download/2026.10.26/yt-dlp_macos.zip",
        sha256: "20d98fa608998eff87d1d35a6e9fb65e4b874639a6cc89addc19da3becde3ef1",
    },
    x86_64: Archive {
        url: "https://github.com/yt-dlp/yt-dlp/releases/download/2026.10.26/yt-dlp_macos.zip",
        sha256: "20d98fa608998eff87d1d35a6e9fb65e4b874639a6cc89addc19da3becde3ef1",
    },
    executable: "yt-dlp_macos",
};

/// Official builds. Checksums from each asset's `.sha256sum` file.
pub const DENO: Tool = Tool {
    name: "deno",
    version: "2.9.8",
    arm64: Archive {
        url: "https://github.com/denoland/deno/releases/download/v2.9.8/deno-aarch64-apple-darwin.zip",
        sha256: "9f0bf660393da2eaff044cdc7444cb08c73f235849b61d224d825a194ce7c98e",
    },
    x86_64: Archive {
        url: "https://github.com/denoland/deno/releases/download/v2.9.8/deno-x86_64-apple-darwin.zip",
        sha256: "4b2f5edd2639b7470f034238b321d7002d8dfb4726d8558f52c17afac370eb2c",
    },
    executable: "deno",
};

/// Static builds by Martin Riedl (https://ffmpeg.martin-riedl.de), configured
/// with `--enable-gpl --enable-version3`, so GPL-3.0-or-later. Build scripts:
/// https://git.martin-riedl.de/ffmpeg/build-script. Checksums from each file's
/// `.sha256`. Provisional choice: the owner confirms the vendor before release.
pub const FFMPEG: Tool = Tool {
    name: "ffmpeg",
    version: "9.0.2",
    arm64: Archive {
        url: "https://ffmpeg.martin-riedl.de/download/macos/arm64/1789931890_9.0.2/ffmpeg.zip",
        sha256: "c8ed4c4e6978a03c485edbfe4e0a5dc2380f8a30bba5150531b31b094492d924",
    },
    x86_64: Archive {
        url: "https://ffmpeg.martin-riedl.de/download/macos/amd64/1789931006_9.0.2/ffmpeg.zip",
        sha256: "7c6b4125b191cbf773832dc51f424cf2b6bb7da43007d1e066f95909e47cacd4",
    },
    executable: "ffmpeg",
};

pub const ALL: [&Tool; 3] = [&YT_DLP, &DENO, &FFMPEG];

#[derive(Debug)]
pub enum ToolError {
    /// The download did not complete.
    Network(String),
    /// The file is not the one that was pinned. It has been deleted.
    ChecksumMismatch,
    /// The file could not be written or unpacked.
    Io(String),
}

impl fmt::Display for ToolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Network(m) | Self::Io(m) => f.write_str(m),
            Self::ChecksumMismatch => f.write_str("checksum mismatch"),
        }
    }
}

impl std::error::Error for ToolError {}

impl Tool {
    fn archive(&self) -> &Archive {
        if cfg!(target_arch = "aarch64") {
            &self.arm64
        } else {
            &self.x86_64
        }
    }

    fn dir(&self, root: &Path) -> PathBuf {
        root.join(format!("{}-{}", self.name, self.version))
    }

    pub fn executable_path(&self, root: &Path) -> PathBuf {
        self.dir(root).join(self.executable)
    }

    /// A tool's folder only appears under its final name once it has been
    /// verified and fully unpacked, so the executable existing is enough.
    pub fn is_installed(&self, root: &Path) -> bool {
        self.executable_path(root).is_file()
    }

    /// Fetches, verifies and unpacks the tool under `root`. Does nothing if it
    /// is already there. `progress` receives bytes received and the total.
    pub async fn install(
        &self,
        root: &Path,
        progress: impl Fn(u64, Option<u64>),
    ) -> Result<(), ToolError> {
        if self.is_installed(root) {
            return Ok(());
        }
        let archive = self.archive();
        let folder = format!("{}-{}", self.name, self.version);
        install_zip(
            root,
            &folder,
            archive.url,
            archive.sha256,
            self.executable,
            progress,
        )
        .await
    }
}

/// Fetches a zip, checks it against `sha256`, and unpacks it as `root/folder`.
/// The folder only appears under its final name once that has all succeeded.
async fn install_zip(
    root: &Path,
    folder: &str,
    url: &str,
    sha256: &str,
    executable: &str,
    progress: impl Fn(u64, Option<u64>),
) -> Result<(), ToolError> {
    if !url.starts_with("https://") {
        return Err(ToolError::Network(
            "tools are fetched over HTTPS only".into(),
        ));
    }
    let io = |e: std::io::Error| ToolError::Io(e.to_string());
    tokio::fs::create_dir_all(root).await.map_err(io)?;

    let zip = root.join(format!("{folder}.zip.part"));
    let client = https_client()?;
    download_verified(&client, url, sha256, &zip, progress).await?;

    // Unpack beside the final place, then rename: a half-unpacked tool is
    // never mistaken for an installed one.
    let partial = root.join(format!("{folder}.partial"));
    let _ = tokio::fs::remove_dir_all(&partial).await;
    let unpacked = tokio::process::Command::new("/usr/bin/ditto")
        .args(["-x", "-k"])
        .arg(&zip)
        .arg(&partial)
        .status()
        .await
        .map_err(io)?;
    let _ = tokio::fs::remove_file(&zip).await;
    if !unpacked.success() || !partial.join(executable).is_file() {
        let _ = tokio::fs::remove_dir_all(&partial).await;
        return Err(ToolError::Io(format!("{folder} could not be unpacked")));
    }
    let _ = tokio::fs::remove_dir_all(root.join(folder)).await;
    tokio::fs::rename(&partial, root.join(folder))
        .await
        .map_err(io)
}

fn https_client() -> Result<reqwest::Client, ToolError> {
    reqwest::Client::builder()
        .https_only(true)
        .timeout(std::time::Duration::from_secs(600))
        .connect_timeout(std::time::Duration::from_secs(20))
        .build()
        .map_err(|e| ToolError::Network(e.to_string()))
}

/// Downloads `url` to `dest` and keeps it only if its SHA-256 is `sha256`.
pub async fn download_verified(
    client: &reqwest::Client,
    url: &str,
    sha256: &str,
    dest: &Path,
    progress: impl Fn(u64, Option<u64>),
) -> Result<(), ToolError> {
    let network = |e: reqwest::Error| ToolError::Network(e.without_url().to_string());
    let io = |e: std::io::Error| ToolError::Io(e.to_string());

    let mut response = client
        .get(url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
        .map_err(network)?;
    let total = response.content_length();
    let mut file = tokio::fs::File::create(dest).await.map_err(io)?;
    let mut hasher = Sha256::new();
    let mut received = 0u64;

    let outcome = async {
        while let Some(chunk) = response.chunk().await.map_err(network)? {
            hasher.update(&chunk);
            file.write_all(&chunk).await.map_err(io)?;
            received += chunk.len() as u64;
            progress(received, total);
        }
        file.flush().await.map_err(io)
    }
    .await;
    drop(file);

    let digest: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let verdict = match outcome {
        Ok(()) if digest.eq_ignore_ascii_case(sha256) => return Ok(()),
        Ok(()) => ToolError::ChecksumMismatch,
        Err(error) => error,
    };
    let _ = tokio::fs::remove_file(dest).await;
    Err(verdict)
}

// ---------------------------------------------------------------- yt-dlp updates
//
// Sites change faster than the app is released, so yt-dlp alone is kept up to
// date on its own: once a day, and when the user asks. The version pinned above
// is only the first install. Each update is checked against the SHA-256 that
// the release itself publishes in `SHA2-256SUMS`, over HTTPS, before it is
// unpacked. (The release also carries a GPG signature of that file, which is
// not checked here.)

/// Which stream of yt-dlp releases to follow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Channel {
    Stable,
    Nightly,
}

impl Channel {
    /// The official repository that publishes this channel's builds.
    fn repo(self) -> &'static str {
        match self {
            Self::Stable => "https://github.com/yt-dlp/yt-dlp",
            Self::Nightly => "https://github.com/yt-dlp/yt-dlp-nightly-builds",
        }
    }
}

const YT_DLP_ASSET: &str = "yt-dlp_macos.zip";
const YT_DLP_STATE: &str = "yt-dlp.json";

/// Which yt-dlp is in use and when updates were last looked for.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct YtDlpState {
    pub version: String,
    pub channel: Channel,
    /// Seconds since 1970.
    pub checked_at: i64,
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// A release tag is used in a folder name, so only a plain one is accepted.
fn plain_tag(tag: &str) -> bool {
    !tag.is_empty()
        && tag.len() <= 40
        && !tag.starts_with('.')
        && tag
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

fn yt_dlp_folder(version: &str) -> String {
    format!("{}-{version}", YT_DLP.name)
}

/// The recorded state, if it names a version that is really on disk.
pub fn yt_dlp_state(root: &Path) -> Option<YtDlpState> {
    let text = std::fs::read_to_string(root.join(YT_DLP_STATE)).ok()?;
    let state: YtDlpState = serde_json::from_str(&text).ok()?;
    let present = plain_tag(&state.version)
        && root
            .join(yt_dlp_folder(&state.version))
            .join(YT_DLP.executable)
            .is_file();
    present.then_some(state)
}

/// The yt-dlp to run: the updated one if there is one, else the pinned first install.
pub fn yt_dlp_executable(root: &Path) -> Option<PathBuf> {
    match yt_dlp_state(root) {
        Some(state) => Some(
            root.join(yt_dlp_folder(&state.version))
                .join(YT_DLP.executable),
        ),
        None => YT_DLP
            .is_installed(root)
            .then(|| YT_DLP.executable_path(root)),
    }
}

pub fn yt_dlp_version(root: &Path) -> Option<String> {
    match yt_dlp_state(root) {
        Some(state) => Some(state.version),
        None => YT_DLP.is_installed(root).then(|| YT_DLP.version.to_owned()),
    }
}

/// The checksum a `SHA2-256SUMS` file gives for one asset.
fn checksum_for(sums: &str, asset: &str) -> Option<String> {
    sums.lines().find_map(|line| {
        let (hash, name) = line.split_once(char::is_whitespace)?;
        let valid = hash.len() == 64 && hash.chars().all(|c| c.is_ascii_hexdigit());
        (name.trim().trim_start_matches('*') == asset && valid).then(|| hash.to_ascii_lowercase())
    })
}

/// The tag of a channel's newest release, read from where its "latest" link leads.
async fn latest_tag(channel: Channel) -> Result<String, ToolError> {
    let network = |e: reqwest::Error| ToolError::Network(e.without_url().to_string());
    let client = reqwest::Client::builder()
        .https_only(true)
        .redirect(reqwest::redirect::Policy::none())
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(network)?;
    let response = client
        .get(format!("{}/releases/latest", channel.repo()))
        .send()
        .await
        .map_err(network)?;
    let tag = response
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|to| to.rsplit_once("/releases/tag/"))
        .map(|(_, tag)| tag.to_owned())
        .filter(|tag| plain_tag(tag));
    tag.ok_or_else(|| ToolError::Network("the newest yt-dlp release could not be found".into()))
}

/// Deletes yt-dlp folders that are no longer the one in use. Run at launch,
/// when no download can be running from an old one.
pub fn remove_old_yt_dlp(root: &Path) {
    let Some(state) = yt_dlp_state(root) else {
        return;
    };
    let keep = yt_dlp_folder(&state.version);
    let prefix = format!("{}-", YT_DLP.name);
    for entry in std::fs::read_dir(root).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with(&prefix) && name != keep && entry.path().is_dir() {
            let _ = std::fs::remove_dir_all(entry.path());
        }
    }
}

/// Looks for a newer yt-dlp on `channel` and installs it if there is one.
/// Returns the version now in use. Nothing happens unless yt-dlp is installed.
pub async fn update_yt_dlp(
    root: &Path,
    channel: Channel,
    progress: impl Fn(u64, Option<u64>),
) -> Result<String, ToolError> {
    let current = yt_dlp_state(root);
    let tag = latest_tag(channel).await?;
    let save = |version: &str| {
        let state = YtDlpState {
            version: version.to_owned(),
            channel,
            checked_at: now(),
        };
        let json = serde_json::to_string(&state).map_err(|e| ToolError::Io(e.to_string()))?;
        std::fs::write(root.join(YT_DLP_STATE), json).map_err(|e| ToolError::Io(e.to_string()))
    };

    let folder = yt_dlp_folder(&tag);
    let on_disk = root.join(&folder).join(YT_DLP.executable).is_file();
    // A stable build may already be on disk as the pinned first install.
    let same = current
        .as_ref()
        .is_some_and(|state| state.version == tag && state.channel == channel);
    if !(same || on_disk && channel == Channel::Stable) {
        let base = format!("{}/releases/download/{tag}", channel.repo());
        let network = |e: reqwest::Error| ToolError::Network(e.without_url().to_string());
        let sums = https_client()?
            .get(format!("{base}/SHA2-256SUMS"))
            .send()
            .await
            .and_then(|r| r.error_for_status())
            .map_err(network)?
            .text()
            .await
            .map_err(network)?;
        let sha256 = checksum_for(&sums, YT_DLP_ASSET)
            .ok_or_else(|| ToolError::Network("the release lists no checksum for yt-dlp".into()))?;
        let url = format!("{base}/{YT_DLP_ASSET}");
        install_zip(root, &folder, &url, &sha256, YT_DLP.executable, progress).await?;
    }
    save(&tag)?;
    Ok(tag)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_checksum_file() {
        let sums = "0f922c15ccc0b83eefa1c8bc9104e434318c4bf16e408e1ee93b7745a6efe0aa  yt-dlp_macos
20D98FA608998EFF87D1D35A6E9FB65E4B874639A6CC89ADDC19DA3BECDE3EF1  yt-dlp_macos.zip
nothex  yt-dlp_other.zip
";
        assert_eq!(
            checksum_for(sums, "yt-dlp_macos.zip").as_deref(),
            Some("20d98fa608998eff87d1d35a6e9fb65e4b874639a6cc89addc19da3becde3ef1")
        );
        assert_eq!(checksum_for(sums, "yt-dlp_other.zip"), None);
        assert_eq!(checksum_for(sums, "missing.zip"), None);
    }

    #[test]
    fn accepts_only_a_plain_release_tag() {
        assert!(plain_tag("2026.10.26"));
        assert!(plain_tag("2026.11.17.232800"));
        assert!(!plain_tag("../../etc"));
        assert!(!plain_tag("a/b"));
        assert!(!plain_tag(".hidden"));
        assert!(!plain_tag(""));
    }

    #[test]
    fn runs_the_updated_yt_dlp_once_there_is_one_and_tidies_the_old() {
        let root = std::env::temp_dir().join(format!("mbiro-ytstate-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let place = |version: &str| {
            let dir = root.join(format!("yt-dlp-{version}"));
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(YT_DLP.executable), b"").unwrap();
        };
        assert_eq!(yt_dlp_executable(&root), None);

        // Only the pinned first install: that is the one to run.
        place(YT_DLP.version);
        assert_eq!(yt_dlp_version(&root).as_deref(), Some(YT_DLP.version));

        // A state file naming a version that is not on disk is ignored.
        let state = |version: &str| {
            let state = YtDlpState {
                version: version.into(),
                channel: Channel::Nightly,
                checked_at: 5,
            };
            std::fs::write(
                root.join(YT_DLP_STATE),
                serde_json::to_string(&state).unwrap(),
            )
            .unwrap();
        };
        state("2099.01.01");
        assert_eq!(yt_dlp_version(&root).as_deref(), Some(YT_DLP.version));

        place("2099.01.01");
        assert_eq!(yt_dlp_version(&root).as_deref(), Some("2099.01.01"));
        assert!(yt_dlp_executable(&root)
            .unwrap()
            .starts_with(root.join("yt-dlp-2099.01.01")));
        assert_eq!(yt_dlp_state(&root).unwrap().channel, Channel::Nightly);

        std::fs::create_dir_all(root.join("deno-2.9.8")).unwrap();
        remove_old_yt_dlp(&root);
        assert!(!root.join(format!("yt-dlp-{}", YT_DLP.version)).exists());
        assert!(root.join("yt-dlp-2099.01.01").exists());
        assert!(
            root.join("deno-2.9.8").exists(),
            "other tools are left alone"
        );

        let _ = std::fs::remove_dir_all(&root);
    }
}
