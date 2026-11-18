//! User-facing messages produced by the core, in one place for translation later.

pub const ENGINE_NOT_RUNNING: &str = "The download engine is not running. Restart the app.";
pub const URL_INVALID: &str = "That does not look like a link. Check it and try again.";
pub const URL_SCHEME_UNSUPPORTED: &str = "Only http and https links can be downloaded.";
pub const REQUEST_INVALID: &str = "The download request could not be read.";
pub const VIDEO_PAUSE_UNAVAILABLE: &str =
    "Video downloads cannot be paused yet. Remove it and add it again.";
pub const TOOLS_MISSING: &str = "The video tools are not installed yet. Fetch them first.";
pub const TOOLS_BUSY: &str = "The video tools are already being fetched.";
pub const TOOL_FETCH_FAILED: &str =
    "A tool could not be downloaded. Check your connection and try again.";
pub const TOOL_CHECKSUM_FAILED: &str =
    "A downloaded tool did not match its checksum and was discarded. Try again later.";
pub const FILE_MISSING: &str = "The file is no longer where it was saved.";
pub const FOLDER_MISSING: &str = "That folder does not exist. Choose another.";
