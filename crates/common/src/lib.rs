//! What the app and the native messaging host must agree on: the names that
//! may still change, where the socket lives, and how a message is framed.
//!
//! Deliberately free of dependencies, so the host stays tiny.

pub mod constants;
pub mod frame;

use std::path::{Path, PathBuf};

/// The Unix socket the running app listens on, for a given home folder.
///
/// A socket path may be at most 104 bytes on macOS; this one leaves room for a
/// user name of about 40 characters.
pub fn socket_path(home: &Path) -> PathBuf {
    home.join("Library/Application Support")
        .join(constants::BUNDLE_IDENTIFIER)
        .join(constants::SOCKET_FILE)
}
