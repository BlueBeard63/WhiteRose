//! A WhiteRose app's connection to whiterosed, the session's settings
//! daemon ($WHITEROSE_DAEMON_SOCKET): every setting when it connects, each
//! change as it happens, and writes. Reconnects if the daemon restarts; an
//! app started outside WhiteRose just runs on defaults.

use std::{
    collections::HashMap,
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};

use serde_json::{json, Value};

/// What changed: every setting (on connecting), or one.
#[derive(Debug, Clone, PartialEq)]
pub enum Change {
    All(HashMap<String, Value>),
    One(String, Value),
}

/// The daemon's socket: $WHITEROSE_DAEMON_SOCKET, else the default name.
pub fn socket_path() -> PathBuf {
    if let Some(p) = std::env::var_os("WHITEROSE_DAEMON_SOCKET") {
        return p.into();
    }
    let runtime = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| "/tmp".into());
    runtime.join("whiterosed.sock")
}

/// Reads one daemon line into a change (None for replies and other events).
pub fn parse(line: &str) -> Option<Change> {
    let v: Value = serde_json::from_str(line).ok()?;
    match v.get("event")?.as_str()? {
        "settings" => {
            let all = v
                .get("values")?
                .as_object()?
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect();
            Some(Change::All(all))
        }
        "setting" => Some(Change::One(
            v.get("key")?.as_str()?.to_string(),
            v.get("value")?.clone(),
        )),
        _ => None,
    }
}

/// A live connection: changes go to `on_change` (on the reader thread).
pub struct Daemon {
    writer: Arc<Mutex<Option<UnixStream>>>,
}

impl Daemon {
    /// Connects in the background and keeps reconnecting (every 2 s) while
    /// the daemon isn't there.
    pub fn start(path: PathBuf, on_change: impl Fn(Change) + Send + 'static) -> Daemon {
        let writer: Arc<Mutex<Option<UnixStream>>> = Arc::new(Mutex::new(None));
        let w = writer.clone();
        std::thread::Builder::new()
            .name("whiterosed".into())
            .spawn(move || loop {
                if let Ok(stream) = UnixStream::connect(&path) {
                    if let Ok(clone) = stream.try_clone() {
                        *w.lock().unwrap() = Some(clone);
                    }
                    for line in BufReader::new(stream).lines() {
                        let Ok(line) = line else { break };
                        if let Some(c) = parse(&line) {
                            on_change(c);
                        }
                    }
                    *w.lock().unwrap() = None;
                }
                std::thread::sleep(Duration::from_secs(2));
            })
            .expect("start the daemon thread");
        Daemon { writer }
    }

    /// Changes a setting (the daemon checks it and tells every app).
    pub fn set(&self, key: &str, value: Value) -> bool {
        let mut guard = self.writer.lock().unwrap();
        let Some(s) = guard.as_mut() else {
            return false;
        };
        let mut line = json!({"op": "set", "key": key, "value": value}).to_string();
        line.push('\n');
        if s.write_all(line.as_bytes()).is_err() {
            *guard = None;
            return false;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{os::unix::net::UnixListener, sync::mpsc, time::Duration};

    #[test]
    fn parses_events_and_ignores_the_rest() {
        assert_eq!(
            parse(r#"{"event":"setting","key":"appearance.theme","value":"light"}"#),
            Some(Change::One("appearance.theme".into(), json!("light")))
        );
        let Some(Change::All(all)) = parse(r#"{"event":"settings","values":{"a":1,"b":"x"}}"#)
        else {
            panic!()
        };
        assert_eq!(all.get("b"), Some(&json!("x")));
        assert_eq!(parse(r#"{"reply":3,"value":1}"#), None);
        assert_eq!(parse(r#"{"event":"state"}"#), None);
        assert_eq!(parse("not json"), None);
    }

    #[test]
    fn follows_changes_and_writes_settings() {
        let dir = std::env::temp_dir().join(format!("wr-kit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let sock = dir.join("d.sock");
        let _ = std::fs::remove_file(&sock);
        let listener = UnixListener::bind(&sock).unwrap();

        let (tx, rx) = mpsc::channel();
        let d = Daemon::start(sock.clone(), move |c| tx.send(c).unwrap());
        let (mut server, _) = listener.accept().unwrap();
        server
            .write_all(b"{\"event\":\"settings\",\"values\":{\"appearance.theme\":\"dark\"}}\n")
            .unwrap();
        server
            .write_all(
                b"{\"event\":\"setting\",\"key\":\"appearance.theme\",\"value\":\"light\"}\n",
            )
            .unwrap();
        assert!(matches!(
            rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            Change::All(_)
        ));
        assert_eq!(
            rx.recv_timeout(Duration::from_secs(2)).unwrap(),
            Change::One("appearance.theme".into(), json!("light"))
        );

        // Writing: the request reaches the daemon.
        for _ in 0..50 {
            if d.set("module.notepad.wrap", json!(false)) {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        let mut got = String::new();
        BufReader::new(server.try_clone().unwrap())
            .read_line(&mut got)
            .unwrap();
        let v: Value = serde_json::from_str(&got).unwrap();
        assert_eq!(
            (v["op"].as_str(), v["key"].as_str(), &v["value"]),
            (Some("set"), Some("module.notepad.wrap"), &json!(false))
        );
        let _ = std::fs::remove_dir_all(dir);
    }
}
