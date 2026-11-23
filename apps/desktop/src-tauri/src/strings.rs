//! User-facing messages produced by the core, in one place for translation later.

pub const ENGINE_NOT_RUNNING: &str = "The download engine is not running. Restart the app.";
pub const URL_INVALID: &str = "That does not look like a link. Check it and try again.";
pub const URL_SCHEME_UNSUPPORTED: &str = "Only http and https links can be downloaded.";
pub const REQUEST_INVALID: &str = "The download request could not be read.";
pub const TOOLS_MISSING: &str = "The video tools are not installed yet. Fetch them first.";
pub const TOOLS_BUSY: &str = "The video tools are already being fetched.";
pub const TOOL_FETCH_FAILED: &str =
    "A tool could not be downloaded. Check your connection and try again.";
pub const TOOL_CHECKSUM_FAILED: &str =
    "A downloaded tool did not match its checksum and was discarded. Try again later.";
pub const FILE_MISSING: &str = "The file is no longer where it was saved.";
pub const FOLDER_MISSING: &str = "That folder does not exist. Choose another.";
pub const NOTIFY_FINISHED: &str = "Download finished";
pub const VIDEO_PROBE_FAILED: &str =
    "No video could be found at that link. Check it and try again.";
pub const VIDEO_PROBE_SLOW: &str = "Reading that page took too long. Try again.";

// Why a download failed. Each is two sentences: what happened, then what to do.
// The list shows the first; the second appears when the pointer rests on it.
pub const FAIL_NOT_FOUND: &str = "The file was not found on the server. Check the link.";
pub const FAIL_NETWORK: &str = "The connection was lost. Retry when you are back online.";
pub const FAIL_NO_SERVER: &str =
    "The server could not be found. Check the link and your connection.";
pub const FAIL_DISK_FULL: &str = "The disk is full. Free some space, then retry.";
pub const FAIL_FILE_EXISTS: &str = "A file with this name is already there. Move it, then retry.";
pub const FAIL_CANNOT_SAVE: &str =
    "The file could not be saved to that folder. Choose another folder.";
pub const FAIL_SIGN_IN: &str =
    "The server asked for a sign-in. Start the download from your browser while signed in.";
pub const FAIL_REFUSED: &str =
    "The server refused the download. Open the link in your browser and start it from there.";
pub const FAIL_REDIRECTS: &str = "The link redirects too many times. Check the link.";
pub const FAIL_NO_RESUME: &str =
    "The server cannot continue a partial download. Remove it and add it again.";
pub const FAIL_OTHER: &str = "The download failed. Retry, or check the link.";
pub const VIDEO_FAIL_PROTECTED: &str =
    "This video is protected and cannot be downloaded. Nothing can be done about that here.";
pub const VIDEO_FAIL_SIGN_IN: &str = "This video needs a sign-in. That is not supported yet.";
pub const VIDEO_FAIL_UNAVAILABLE: &str = "This video is not available. Check the link.";
pub const VIDEO_FAIL_OTHER: &str = "The video could not be downloaded. Retry, or check the link.";

pub const LICENCE_UNAVAILABLE: &str = "Licence keys cannot be checked in this build yet.";
pub const LICENCE_INVALID: &str =
    "That key is not valid. Copy all of it again from your licence email.";
pub const LICENCE_OFFLINE: &str =
    "The licence server could not be reached. Check your connection and try again.";
pub const LICENCE_DEVICE_LIMIT: &str =
    "This key is already in use on 3 Macs. Choose Remove Licence in Settings on one of them, then try again.";
pub const LICENCE_REVOKED: &str = "This licence was refunded and no longer works.";
pub const LICENCE_SERVICE: &str =
    "The licence server gave an answer this app does not understand. Try again later.";
pub const LICENCE_NO_DEVICE: &str =
    "This Mac could not be identified, so the key cannot be activated.";
pub const LICENCE_REMOVE_OFFLINE: &str =
    "The licence server could not be reached, so this Mac's place on the key cannot be freed. Check your connection and try again.";
pub const UPDATE_OFF: &str = "Updates are not switched on in this build yet.";
pub const UPDATE_CHECK_FAILED: &str =
    "Could not check for updates. Check your connection and try again.";
pub const UPDATE_INSTALL_FAILED: &str =
    "The update could not be installed. Nothing was changed; try again later.";
pub const UPDATE_NONE: &str = "This is already the newest version.";
pub const UPDATE_BUSY: &str = "An update is already being installed.";
pub const ACK_ARIA2_ROLE: &str =
    "Does the downloading. It is a separate program inside the app, started and stopped by it.";
pub const ACK_ARIA2_SOURCE: &str =
    "aria2 is free software under the GNU General Public License, version 2 or later. The copy inside this app was built, unchanged, from this source:";
pub const ACK_ARIA2_BUILD: &str =
    "The licence follows, then the script that built it, so that you can build the same program yourself.";
pub const ACK_FONT_ROLE: &str = "A typeface used in the window.";
pub const ACK_YT_DLP_ROLE: &str =
    "Finds and fetches video from a page. Kept up to date by the app, so its version is shown under Video tools in Settings.";
pub const ACK_DENO_ROLE: &str = "Runs the scripts yt-dlp needs for some pages.";
pub const ACK_FFMPEG_ROLE: &str = "Joins separate video and sound into one file.";
pub const TRIAL_ENDED_VIDEO: &str = "The trial has ended. Video downloads need a licence.";
