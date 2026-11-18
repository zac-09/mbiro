//! Names used by the desktop core. The ones that may still change live in
//! `mbiro-common`, shared with the native messaging host.

pub use mbiro_common::constants::*;

/// File names of the bundled helpers, next to the main executable.
pub const ARIA2_BINARY: &str = "aria2c";
pub const NM_HOST_BINARY: &str = "nm-host";

/// Event carrying the full list of jobs, sent once a second.
pub const SNAPSHOT_EVENT: &str = "downloads:snapshot";
