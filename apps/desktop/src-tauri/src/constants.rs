//! The one Rust home for names that may still change.
//!
//! All four are placeholders until the owner confirms them (PLAN.md section 14).
//! The TypeScript twin is `packages/protocol/src/constants.ts`. The product name
//! and identifier also appear in `tauri.conf.json`, which Tauri reads at build time.
#![allow(dead_code)]

pub const PRODUCT_NAME: &str = "Mbiro";
pub const BUNDLE_IDENTIFIER: &str = "dev.mbiro.app";
pub const NATIVE_HOST_NAME: &str = "dev.mbiro.host";
pub const URL_SCHEME: &str = "mbiro";

/// File name of the bundled aria2 sidecar, next to the main executable.
pub const ARIA2_BINARY: &str = "aria2c";

/// Event carrying the full list of jobs, sent once a second.
pub const SNAPSHOT_EVENT: &str = "downloads:snapshot";
