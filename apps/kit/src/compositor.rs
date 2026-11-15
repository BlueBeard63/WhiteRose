//! Requests to the WhiteRose compositor ($WHITEROSE_SOCKET, set for every
//! process in a WhiteRose session): one line of JSON, nothing back.

use std::{
    io::Write,
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    time::Duration,
};

/// The compositor's socket, outside a WhiteRose session none.
pub fn socket_path() -> Option<PathBuf> {
    std::env::var_os("WHITEROSE_SOCKET").map(PathBuf::from)
}

/// How long the connection stays open after the request: the compositor
/// first writes its state to a new client, and a client already gone by
/// then is dropped before its request is read.
const LINGER: Duration = Duration::from_secs(2);

/// Sends `request` (one line of JSON) to the compositor at `path`, on a
/// thread of its own so the UI never waits. False if it can't connect.
pub fn send(path: &Path, request: &str) -> bool {
    let Ok(mut stream) = UnixStream::connect(path) else {
        return false;
    };
    let line = format!("{}\n", request.trim_end());
    std::thread::spawn(move || {
        if stream.write_all(line.as_bytes()).is_ok() {
            std::thread::sleep(LINGER);
        }
    });
    true
}

/// Asks for the snap layout picker under the pointer (a WhiteRose app's
/// maximize button was hovered). False outside WhiteRose.
pub fn snap_picker() -> bool {
    socket_path().is_some_and(|p| send(&p, r#"{"cmd":"snap_picker","under_pointer":true}"#))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;
    use std::{io::BufRead, os::unix::net::UnixListener};

    #[test]
    fn sends_one_line_and_waits_for_the_compositor() {
        let dir = Scratch::new("kit-compositor");
        let path = dir.join("sock");
        let _ = std::fs::remove_file(&path);
        let listener = UnixListener::bind(&path).unwrap();
        assert!(send(&path, r#"{"cmd":"snap_picker","under_pointer":true}"#));
        let (stream, _) = listener.accept().unwrap();
        // The compositor writes its state first: the kit is still there.
        (&stream).write_all(b"{\"event\":\"focus\"}\n").unwrap();
        let mut line = String::new();
        std::io::BufReader::new(&stream)
            .read_line(&mut line)
            .unwrap();
        assert_eq!(line, "{\"cmd\":\"snap_picker\",\"under_pointer\":true}\n");
        assert!(!send(&dir.join("nothing"), "{}"), "no compositor there");
    }
}
