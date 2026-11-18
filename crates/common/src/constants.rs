//! The one Rust home for names that may still change.
//!
//! All of them are placeholders until the owner confirms them (PLAN.md section
//! 14). The TypeScript twin is `packages/protocol/src/constants.ts`. The product
//! name and identifier also appear in `tauri.conf.json`, which Tauri reads at
//! build time.

pub const PRODUCT_NAME: &str = "Mbiro";
pub const BUNDLE_IDENTIFIER: &str = "dev.mbiro.app";
pub const NATIVE_HOST_NAME: &str = "dev.mbiro.host";
pub const URL_SCHEME: &str = "mbiro";

/// Id of the development build of the Chrome extension, fixed by the public
/// `key` in `apps/extension/wxt.config.ts`. The store assigns its own id later.
pub const CHROME_EXTENSION_ID_DEV: &str = "jcggnjbklbbladijcfhnbhnihdpahjga";
/// Id the Firefox extension declares for itself.
pub const FIREFOX_EXTENSION_ID: &str = "extension@mbiro.dev";

/// File name of the app's socket inside its Application Support folder.
pub const SOCKET_FILE: &str = "host.sock";
