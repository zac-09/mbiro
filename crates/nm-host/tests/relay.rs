//! Runs the real host binary the way a browser does: framed JSON on stdin,
//! framed JSON back on stdout, with a stand-in for the app on the socket.

use std::io::Write;
use std::os::unix::net::UnixListener;
use std::path::PathBuf;
use std::process::{Command, Stdio};

use mbiro_common::frame::{read_frame, write_frame};

/// A short home folder: socket paths are limited to 104 bytes.
fn fake_home(name: &str) -> PathBuf {
    let home = PathBuf::from(format!("/tmp/mbiro-nmh-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(mbiro_common::socket_path(&home).parent().unwrap()).unwrap();
    home
}

fn host(home: &PathBuf) -> std::process::Child {
    Command::new(env!("CARGO_BIN_EXE_mbiro-nm-host"))
        .env("HOME", home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("start the host")
}

#[test]
fn relays_each_message_to_the_app_and_the_answer_back() {
    let home = fake_home("relay");
    let listener = UnixListener::bind(mbiro_common::socket_path(&home)).unwrap();
    let app = std::thread::spawn(move || {
        let mut seen = Vec::new();
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let message = read_frame(&mut stream).unwrap().unwrap();
            let reply = format!(r#"{{"echo":{}}}"#, message.len());
            write_frame(&mut stream, reply.as_bytes()).unwrap();
            seen.push(message);
        }
        seen
    });

    let mut child = host(&home);
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let first = br#"{"type":"download","version":1,"url":"http://127.0.0.1/a.bin"}"#;
    let second = br#"{"type":"download","version":1,"url":"http://127.0.0.1/b.bin"}"#;

    write_frame(&mut stdin, first).unwrap();
    let reply = read_frame(&mut stdout).unwrap().unwrap();
    assert_eq!(reply, format!(r#"{{"echo":{}}}"#, first.len()).as_bytes());
    write_frame(&mut stdin, second).unwrap();
    assert!(read_frame(&mut stdout).unwrap().is_some());

    drop(stdin); // the browser disconnects
    assert!(child.wait().unwrap().success());
    assert_eq!(app.join().unwrap(), vec![first.to_vec(), second.to_vec()]);
    let _ = std::fs::remove_dir_all(&home);
}

#[test]
fn says_so_when_the_app_is_not_running() {
    let home = fake_home("closed");
    let mut child = host(&home);
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();

    write_frame(
        &mut stdin,
        br#"{"type":"download","version":1,"url":"http://127.0.0.1/a"}"#,
    )
    .unwrap();
    let reply = read_frame(&mut stdout).unwrap().unwrap();
    assert_eq!(
        reply,
        br#"{"type":"ack","ok":false,"code":"app_not_running"}"#
    );

    stdin.flush().unwrap();
    drop(stdin);
    assert!(child.wait().unwrap().success());
    let _ = std::fs::remove_dir_all(&home);
}
