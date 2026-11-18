//! Native messaging host. The browser starts it and talks to it on stdin and
//! stdout; it passes each message, untouched, to the running app over a Unix
//! socket and passes the app's answer back.
//!
//! It does not parse or log messages: checking them is the app's job.
//!
//! Not yet here (M2): launching the app when it is closed.

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

fn run(socket: &Path, mut input: impl Read, mut output: impl Write) -> io::Result<()> {
    // The browser closes stdin when the extension is done; that ends the loop.
    while let Some(message) = read_frame(&mut input)? {
        let reply = relay(socket, &message).unwrap_or_else(|_| APP_NOT_RUNNING.to_vec());
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
