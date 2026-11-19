//! Native messaging host. The browser starts it and talks to it on stdin and
//! stdout; it passes each message, untouched, to the running app over a Unix
//! socket and passes the app's answer back.
//!
//! It does not parse or log messages: checking them is the app's job.
//!
//! The one message it acts on itself is "open": if the app is closed, the host
//! starts it first.

use std::io::{self, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

use mbiro_common::frame::{read_frame, write_frame};

/// Sent to the extension when the app's socket cannot be reached.
/// Must match `ackSchema` in `packages/protocol`.
const APP_NOT_RUNNING: &[u8] = br#"{"type":"ack","ok":false,"code":"app_not_running"}"#;
const APP_TIMEOUT: Duration = Duration::from_secs(10);

fn relay(socket: &Path, message: &[u8]) -> io::Result<Vec<u8>> {
    let mut stream = UnixStream::connect(socket)?;
    stream.set_read_timeout(Some(APP_TIMEOUT))?;
    stream.set_write_timeout(Some(APP_TIMEOUT))?;
    write_frame(&mut stream, message)?;
    read_frame(&mut stream)?.ok_or_else(|| io::ErrorKind::UnexpectedEof.into())
}

/// Marks the one request the host acts on itself: "open the app".
/// Everything else is passed on unread. Must match `openRequestSchema` in
/// `packages/protocol`.
const OPEN_REQUEST_MARK: &[u8] = br#""type":"open""#;
/// How long a freshly launched app gets to open its socket.
const LAUNCH_WAIT: Duration = Duration::from_secs(12);

fn is_open_request(message: &[u8]) -> bool {
    message
        .windows(OPEN_REQUEST_MARK.len())
        .any(|window| window == OPEN_REQUEST_MARK)
}

/// The app bundle this host sits in (`X.app/Contents/MacOS/nm-host`), if it does.
/// A development build is not inside a bundle and cannot be launched this way.
fn enclosing_app(host: &Path) -> Option<PathBuf> {
    let bundle = host.parent()?.parent()?.parent()?;
    let is_app = bundle.extension().is_some_and(|ext| ext == "app");
    (is_app && host.parent()?.ends_with("Contents/MacOS")).then(|| bundle.to_owned())
}

/// Starts the app through Launch Services, so it is not a child of the browser,
/// and waits for its socket.
fn launch_app(socket: &Path) -> bool {
    let Some(app) = std::env::current_exe()
        .ok()
        .as_deref()
        .and_then(enclosing_app)
    else {
        return false;
    };
    let started = std::process::Command::new("/usr/bin/open")
        .arg(&app)
        .status();
    if !started.is_ok_and(|status| status.success()) {
        return false;
    }
    let deadline = std::time::Instant::now() + LAUNCH_WAIT;
    while std::time::Instant::now() < deadline {
        if UnixStream::connect(socket).is_ok() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(250));
    }
    false
}

fn run(socket: &Path, mut input: impl Read, mut output: impl Write) -> io::Result<()> {
    // The browser closes stdin when the extension is done; that ends the loop.
    while let Some(message) = read_frame(&mut input)? {
        let reply = match relay(socket, &message) {
            Ok(reply) => reply,
            // The app is closed. It is only started when the user asked for it.
            Err(_) if is_open_request(&message) && launch_app(socket) => {
                relay(socket, &message).unwrap_or_else(|_| APP_NOT_RUNNING.to_vec())
            }
            Err(_) => APP_NOT_RUNNING.to_vec(),
        };
        write_frame(&mut output, &reply)?;
    }
    Ok(())
}

fn main() {
    let Some(home) = std::env::var_os("HOME").map(PathBuf::from) else {
        std::process::exit(2);
    };
    let socket = mbiro_common::socket_path(&home);
    if run(&socket, io::stdin().lock(), io::stdout().lock()).is_err() {
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_bundle_only_from_inside_one() {
        assert_eq!(
            enclosing_app(Path::new("/Applications/Mbiro.app/Contents/MacOS/nm-host")),
            Some(PathBuf::from("/Applications/Mbiro.app"))
        );
        assert_eq!(enclosing_app(Path::new("/code/target/debug/nm-host")), None);
        assert_eq!(
            enclosing_app(Path::new("/x/Fake.app/bin/sub/nm-host")),
            None
        );
    }

    #[test]
    fn recognises_only_the_open_request() {
        assert!(is_open_request(br#"{"type":"open","version":1}"#));
        assert!(!is_open_request(
            br#"{"type":"download","version":1,"url":"http://x/open"}"#
        ));
    }
}
