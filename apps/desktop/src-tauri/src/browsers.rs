//! Tells browsers where the native messaging host is, by writing a small
//! manifest into each installed browser's folder (PLAN.md section 5).
//!
//! Covers Chrome, Edge, Brave and Firefox. Until the extension is in the stores
//! only the development build's id is allowed; Edge's store assigns its own id,
//! separate from Chrome's, and both must be listed here once they exist.

use std::io;
use std::path::{Path, PathBuf};

use serde_json::json;

use crate::constants::{
    CHROME_EXTENSION_ID_DEV, FIREFOX_EXTENSION_ID, NATIVE_HOST_NAME, PRODUCT_NAME,
};

struct Browser {
    /// What the browser is called, for the Settings screen.
    name: &'static str,
    /// Folder under `~/Library/Application Support` that exists once the browser has run.
    home: &'static str,
    /// Where it looks for host manifests, under the same root.
    manifests: &'static str,
    /// The manifest key naming who may connect, and the value for our extension.
    allow_key: &'static str,
    allow_value: fn() -> String,
}

/// Chrome, Edge and Brave share one engine and one manifest format.
fn chromium_origin() -> String {
    format!("chrome-extension://{CHROME_EXTENSION_ID_DEV}/")
}

const BROWSERS: [Browser; 4] = [
    Browser {
        name: "Chrome",
        home: "Google/Chrome",
        manifests: "Google/Chrome/NativeMessagingHosts",
        allow_key: "allowed_origins",
        allow_value: chromium_origin,
    },
    Browser {
        name: "Edge",
        home: "Microsoft Edge",
        manifests: "Microsoft Edge/NativeMessagingHosts",
        allow_key: "allowed_origins",
        allow_value: chromium_origin,
    },
    Browser {
        name: "Brave",
        home: "BraveSoftware/Brave-Browser",
        manifests: "BraveSoftware/Brave-Browser/NativeMessagingHosts",
        allow_key: "allowed_origins",
        allow_value: chromium_origin,
    },
    Browser {
        // Firefox keeps profiles under `Firefox` but reads manifests from `Mozilla`.
        name: "Firefox",
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

/// One browser as the Settings screen shows it.
#[derive(Debug, PartialEq, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserStatus {
    pub name: &'static str,
    /// The browser has been run on this Mac.
    pub installed: bool,
    /// It has been told where the native messaging host is.
    pub connected: bool,
}

pub fn status(home: &Path) -> Vec<BrowserStatus> {
    let support = home.join("Library/Application Support");
    BROWSERS
        .iter()
        .map(|browser| BrowserStatus {
            name: browser.name,
            installed: support.join(browser.home).is_dir(),
            connected: support
                .join(browser.manifests)
                .join(format!("{NATIVE_HOST_NAME}.json"))
                .is_file(),
        })
        .collect()
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

        let seen = status(&home);
        assert_eq!(seen.len(), 4);
        assert_eq!(
            seen[0],
            BrowserStatus {
                name: "Chrome",
                installed: true,
                connected: true
            }
        );
        assert_eq!(
            seen[1],
            BrowserStatus {
                name: "Edge",
                installed: false,
                connected: false
            }
        );

        // Edge and Brave get the same manifest as Chrome, each in its own folder.
        std::fs::create_dir_all(support.join("Microsoft Edge")).unwrap();
        std::fs::create_dir_all(support.join("BraveSoftware/Brave-Browser")).unwrap();
        let written = install_manifests(&home, Path::new("/x/nm-host")).unwrap();
        assert_eq!(written.len(), 4);
        for folder in ["Microsoft Edge", "BraveSoftware/Brave-Browser"] {
            let file = support
                .join(folder)
                .join("NativeMessagingHosts/dev.mbiro.host.json");
            assert!(written.contains(&file), "{folder} got no manifest");
            let manifest: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
            assert_eq!(
                manifest["allowed_origins"][0],
                "chrome-extension://jcggnjbklbbladijcfhnbhnihdpahjga/"
            );
            assert_eq!(manifest["path"], "/x/nm-host");
        }

        let _ = std::fs::remove_dir_all(&home);
    }
}
