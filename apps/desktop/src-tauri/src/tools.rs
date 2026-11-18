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
        if !archive.url.starts_with("https://") {
            return Err(ToolError::Network(
                "tools are fetched over HTTPS only".into(),
            ));
        }
        let io = |e: std::io::Error| ToolError::Io(e.to_string());
        tokio::fs::create_dir_all(root).await.map_err(io)?;

        let zip = root.join(format!("{}-{}.zip.part", self.name, self.version));
        let client = reqwest::Client::builder()
            .https_only(true)
            .build()
            .map_err(|e| ToolError::Network(e.to_string()))?;
        download_verified(&client, archive.url, archive.sha256, &zip, progress).await?;

        // Unpack beside the final place, then rename: a half-unpacked tool is
        // never mistaken for an installed one.
        let partial = root.join(format!("{}-{}.partial", self.name, self.version));
        let _ = tokio::fs::remove_dir_all(&partial).await;
        let unpacked = tokio::process::Command::new("/usr/bin/ditto")
            .args(["-x", "-k"])
            .arg(&zip)
            .arg(&partial)
            .status()
            .await
            .map_err(io)?;
        let _ = tokio::fs::remove_file(&zip).await;
        if !unpacked.success() || !partial.join(self.executable).is_file() {
            let _ = tokio::fs::remove_dir_all(&partial).await;
            return Err(ToolError::Io(format!(
                "{} could not be unpacked",
                self.name
            )));
        }
        let _ = tokio::fs::remove_dir_all(self.dir(root)).await;
        tokio::fs::rename(&partial, self.dir(root))
            .await
            .map_err(io)
    }
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
