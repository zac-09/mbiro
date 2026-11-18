//! Names used by the desktop core. The ones that may still change live in
//! `mbiro-common`, shared with the native messaging host.

pub use mbiro_common::constants::*;

/// File names of the bundled helpers, next to the main executable.
pub const ARIA2_BINARY: &str = "aria2c";
pub const NM_HOST_BINARY: &str = "nm-host";

/// Event carrying the full list of jobs, sent once a second.
pub const SNAPSHOT_EVENT: &str = "downloads:snapshot";

/// Event carrying the download progress of one helper tool.
pub const TOOL_PROGRESS_EVENT: &str = "tools:progress";

/// Event carrying one job that has just finished.
pub const FINISHED_EVENT: &str = "downloads:finished";

/// Window labels and the menu bar item's id. The UI reads the label to know
/// which screen to draw (`apps/desktop/src/main.tsx`).
pub const MAIN_WINDOW: &str = "main";
pub const MENUBAR_WINDOW: &str = "menubar";
pub const TRAY_ID: &str = "menubar";
