//! Updating the app itself.
//!
//! The app reads a small JSON manifest from static hosting. If it names a newer
//! version, the app downloads that version, checks its signature against the
//! public key below, and only then puts it in place of the running copy. The
//! work is `tauri-plugin-updater`'s; this file decides when, and with what.
//!
//! Both values below are `None` until the owner sets them, and until both are
//! set the app never checks: nothing is fetched and nothing can be installed.

use serde::Serialize;
use tauri::AppHandle;
use tauri_plugin_updater::{Updater, UpdaterExt};
use url::Url;

/// The key updates are checked against: the contents of the `.pub` file made
/// by `tauri signer generate`. The private half signs each release in
/// `scripts/release.sh` and never comes near this repository.
pub const PUBLIC_KEY: Option<&str> = None;

/// Where the manifest lives, as an `https://` address ending in `latest.json`.
pub const MANIFEST_URL: Option<&str> = None;

/// The name the one universal build goes by in the manifest's `platforms`.
/// `scripts/release.sh` writes the same name.
pub const TARGET: &str = "darwin-universal";

/// Whether this build can update itself at all.
pub fn configured() -> bool {
    PUBLIC_KEY.is_some() && MANIFEST_URL.is_some()
}

#[derive(Debug, PartialEq)]
pub enum UpdateError {
    /// This build names no key or no manifest.
    NotConfigured,
    /// The manifest could not be fetched or read.
    CheckFailed,
    /// The download failed, or its signature was not ours, or it could not be
    /// put in place. The running copy is untouched in every case.
    InstallFailed,
    /// Asked to install, but there is nothing newer.
    UpToDate,
}

/// A newer version, as the window shows it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Found {
    pub version: String,
    pub notes: Option<String>,
}

fn updater(app: &AppHandle) -> Result<Updater, UpdateError> {
    let (Some(key), Some(manifest)) = (PUBLIC_KEY, MANIFEST_URL) else {
        return Err(UpdateError::NotConfigured);
    };
    let manifest = Url::parse(manifest).map_err(|_| UpdateError::NotConfigured)?;
    app.updater_builder()
        .pubkey(key)
        .endpoints(vec![manifest])
        .and_then(|builder| builder.target(TARGET).build())
        .map_err(|_| UpdateError::NotConfigured)
}

/// Asks the manifest whether there is a newer version.
pub async fn check(app: &AppHandle) -> Result<Option<Found>, UpdateError> {
    let update = updater(app)?
        .check()
        .await
        .map_err(|_| UpdateError::CheckFailed)?;
    Ok(update.map(|update| Found {
        version: update.version,
        notes: update.body.filter(|notes| !notes.trim().is_empty()),
    }))
}

/// Downloads the newer version, checks its signature and puts it in place.
/// The new copy runs from the next launch. `progress` is called with the bytes
/// received so far and the total, when the server says it.
pub async fn install(
    app: &AppHandle,
    mut progress: impl FnMut(u64, Option<u64>),
) -> Result<Found, UpdateError> {
    let update = updater(app)?
        .check()
        .await
        .map_err(|_| UpdateError::CheckFailed)?
        .ok_or(UpdateError::UpToDate)?;
    let mut received = 0u64;
    update
        .download_and_install(
            |chunk, total| {
                received += chunk as u64;
                progress(received, total);
            },
            || {},
        )
        .await
        .map_err(|_| UpdateError::InstallFailed)?;
    Ok(Found {
        version: update.version,
        notes: None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tauri_plugin_updater::RemoteRelease;

    /// The manifest exactly as `scripts/release.sh` writes it.
    const MANIFEST: &str = r#"{
  "version": "1.2.0",
  "notes": "",
  "pub_date": "2026-11-23T10:00:00Z",
  "platforms": {
    "darwin-universal": {
      "signature": "c2lnbmF0dXJl",
      "url": "https://example.com/updates/Mbiro-1.2.0-universal.app.tar.gz"
    }
  }
}"#;

    #[test]
    fn the_release_script_s_manifest_is_one_the_updater_reads() {
        let release: RemoteRelease = serde_json::from_str(MANIFEST).expect("a manifest");
        assert_eq!(release.version.to_string(), "1.2.0");
        assert_eq!(
            release.download_url(TARGET).expect("our build").as_str(),
            "https://example.com/updates/Mbiro-1.2.0-universal.app.tar.gz"
        );
        assert_eq!(release.signature(TARGET).expect("signed"), "c2lnbmF0dXJl");
        // A build for anything else is not there to be picked up by mistake.
        assert!(release.download_url("darwin-aarch64").is_err());
    }

    #[test]
    fn updates_stay_off_until_both_values_are_set() {
        assert_eq!(configured(), PUBLIC_KEY.is_some() && MANIFEST_URL.is_some());
        if let Some(manifest) = MANIFEST_URL {
            let url = Url::parse(manifest).expect("a URL");
            assert_eq!(url.scheme(), "https");
        }
    }
}
