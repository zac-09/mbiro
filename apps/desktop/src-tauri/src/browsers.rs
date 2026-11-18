//! Tells browsers where the native messaging host is, by writing a small
//! manifest into each installed browser's folder (PLAN.md section 5).
//!
//! Spike 4 covers Chrome and Firefox. Edge and Brave follow in M2.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::json;

use crate::constants::{
    CHROME_EXTENSION_ID_DEV, FIREFOX_EXTENSION_ID, NATIVE_HOST_NAME, PRODUCT_NAME,
};

struct Browser {
    /// Folder under `~/Library/Application Support` that exists once the browser has run.
    home: &'static str,
    /// Where it looks for host manifests, under the same root.
    manifests: &'static str,
    /// The manifest key naming who may connect, and the value for our extension.
    allow_key: &'static str,
    allow_value: fn() -> String,
}

const BROWSERS: [Browser; 2] = [
    Browser {
        home: "Google/Chrome",
        manifests: "Google/Chrome/NativeMessagingHosts",
        allow_key: "allowed_origins",
        allow_value: || format!("chrome-extension://{CHROME_EXTENSION_ID_DEV}/"),
    },
    Browser {
        // Firefox keeps profiles under `Firefox` but reads manifests from `Mozilla`.
        home: "Firefox",
        manifests: "Mozilla/NativeMessagingHosts",
        allow_key: "allowed_extensions",
        allow_value: || FIREFOX_EXTENSION_ID.to_owned(),
    },
];

/// Writes the manifest for every supported browser found under `home`, and
/// returns the files written. A browser that is not installed is left alone.
///
/// The manifest holds the host's absolute path, so this runs on every launch:
/// moving the app would otherwise leave the browsers pointing at nothing.
pub fn install_manifests(home: &Path, host_binary: &Path) -> io::Result<Vec<PathBuf>> {
    let support = home.join("Library/Application Support");
    let mut written = Vec::new();
    for browser in &BROWSERS {
        if !support.join(browser.home).is_dir() {
            continue;
        }
        let manifest = json!({
            "name": NATIVE_HOST_NAME,
            "description": PRODUCT_NAME,
            "path": host_binary,
            "type": "stdio",
            browser.allow_key: [(browser.allow_value)()],
        });
        let dir = support.join(browser.manifests);
        std::fs::create_dir_all(&dir)?;
        let file = dir.join(format!("{NATIVE_HOST_NAME}.json"));
        std::fs::write(&file, serde_json::to_vec_pretty(&manifest)?)?;
        written.push(file);
    }
    Ok(written)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    #[test]
    fn writes_a_manifest_only_for_browsers_that_are_installed() {
        let home = std::env::temp_dir().join(format!("mbiro-browsers-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&home);
        let support = home.join("Library/Application Support");
        std::fs::create_dir_all(support.join("Google/Chrome")).unwrap();

        let written = install_manifests(&home, Path::new("/Applications/X.app/nm-host")).unwrap();
        assert_eq!(
            written,
            vec![support.join("Google/Chrome/NativeMessagingHosts/dev.mbiro.host.json")]
        );
        assert!(
            !support.join("Mozilla").exists(),
            "no folder is made for a missing browser"
        );

        let manifest: Value = serde_json::from_slice(&std::fs::read(&written[0]).unwrap()).unwrap();
        assert_eq!(manifest["name"], "dev.mbiro.host");
        assert_eq!(manifest["type"], "stdio");
        assert_eq!(manifest["path"], "/Applications/X.app/nm-host");
        assert_eq!(
            manifest["allowed_origins"][0],
            "chrome-extension://jcggnjbklbbladijcfhnbhnihdpahjga/"
        );

        std::fs::create_dir_all(support.join("Firefox")).unwrap();
        let written = install_manifests(&home, Path::new("/x/nm-host")).unwrap();
        assert_eq!(written.len(), 2);
        let firefox: Value = serde_json::from_slice(&std::fs::read(&written[1]).unwrap()).unwrap();
        assert_eq!(firefox["allowed_extensions"][0], "extension@mbiro.dev");
        assert!(firefox.get("allowed_origins").is_none());

        let _ = std::fs::remove_dir_all(&home);
    }
}
