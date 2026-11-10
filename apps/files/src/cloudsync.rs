//! Folders kept in sync by a desktop sync client (Nextcloud, ownCloud):
//! each file's state as a badge (synced, syncing, waiting to upload,
//! error, online only), and the client's own actions (keep on this
//! device, free up space, share, copy its link).
//!
//! These clients take requests on a local socket, the one Nautilus and
//! Dolphin talk to ($XDG_RUNTIME_DIR/Nextcloud/socket): when something
//! connects they say which folders they sync ("REGISTER_PATH:/home/me/
//! Nextcloud"), and answer "RETRIEVE_FILE_STATUS:<path>" with
//! "STATUS:<state>:<path>". With virtual files on, a file that's only in
//! the cloud is a placeholder named "<name>.nextcloud" (or ".owncloud").

use std::{
    collections::BTreeMap,
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    path::{Path, PathBuf},
    time::{Duration, Instant},
};

use serde::Serialize;

/// The clients Files knows, and where their sockets are.
pub fn clients(runtime: &Path) -> Vec<(&'static str, PathBuf, &'static str)> {
    vec![
        ("Nextcloud", runtime.join("Nextcloud/socket"), ".nextcloud"),
        ("ownCloud", runtime.join("ownCloud/socket"), ".owncloud"),
    ]
}

#[derive(Debug, Clone, Serialize, PartialEq, Default)]
pub struct Status {
    /// Which client ("" if the folder isn't synced).
    pub client: String,
    /// The synced folder it's in.
    pub root: String,
    /// name -> synced | syncing | new | error | ignored | online
    pub entries: BTreeMap<String, String>,
}

fn state(word: &str) -> Option<&'static str> {
    // "+SWM" marks a shared item; the state is before it.
    Some(match word.split('+').next()? {
        "OK" => "synced",
        "SYNC" => "syncing",
        "NEW" => "new",
        "ERROR" => "error",
        "IGNORE" => "ignored",
        _ => return None,
    })
}

struct Conn {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
}

impl Conn {
    fn open(sock: &Path) -> Option<Conn> {
        let stream = UnixStream::connect(sock).ok()?;
        stream
            .set_read_timeout(Some(Duration::from_millis(150)))
            .ok()?;
        stream
            .set_write_timeout(Some(Duration::from_secs(1)))
            .ok()?;
        let reader = BufReader::new(stream.try_clone().ok()?);
        Some(Conn { stream, reader })
    }
    fn send(&mut self, line: &str) -> bool {
        self.stream
            .write_all(format!("{line}\n").as_bytes())
            .is_ok()
    }
    /// The next line within the read timeout.
    fn line(&mut self) -> Option<String> {
        let mut s = String::new();
        match self.reader.read_line(&mut s) {
            Ok(n) if n > 0 => Some(s.trim_end_matches(['\n', '\r']).to_string()),
            _ => None,
        }
    }
    /// The folders it syncs (said as soon as something connects).
    fn roots(&mut self) -> Vec<PathBuf> {
        let mut out = vec![];
        let until = Instant::now() + Duration::from_millis(400);
        while Instant::now() < until {
            match self.line() {
                Some(l) => {
                    if let Some(p) = l.strip_prefix("REGISTER_PATH:") {
                        out.push(PathBuf::from(p));
                    }
                }
                None if !out.is_empty() => break,
                None => {}
            }
        }
        out
    }
}

/// The sync state of what's in `dir` (None if no client syncs it). At
/// most `limit` entries are asked about (a huge folder isn't held up).
pub fn status(runtime: &Path, dir: &Path, limit: usize) -> Option<Status> {
    for (name, sock, suffix) in clients(runtime) {
        if !sock.exists() {
            continue;
        }
        let Some(mut c) = Conn::open(&sock) else {
            continue;
        };
        let Some(root) = c.roots().into_iter().find(|r| dir.starts_with(r)) else {
            continue;
        };
        let mut st = Status {
            client: name.into(),
            root: root.to_string_lossy().into_owned(),
            entries: BTreeMap::new(),
        };
        let mut asked: Vec<(String, PathBuf)> = vec![];
        if let Ok(rd) = std::fs::read_dir(dir) {
            for e in rd.flatten().take(limit) {
                let n = e.file_name().to_string_lossy().into_owned();
                // A placeholder: in the cloud only.
                if n.ends_with(suffix) && e.file_type().map(|t| t.is_file()).unwrap_or(false) {
                    st.entries.insert(n, "online".into());
                    continue;
                }
                let p = e.path();
                let ask = if e.file_type().map(|t| t.is_dir()).unwrap_or(false) {
                    "RETRIEVE_FOLDER_STATUS"
                } else {
                    "RETRIEVE_FILE_STATUS"
                };
                if !c.send(&format!("{ask}:{}", p.display())) {
                    break;
                }
                asked.push((n, p));
            }
        }
        // Answers come in any order, among other messages.
        let want: BTreeMap<PathBuf, String> = asked.into_iter().map(|(n, p)| (p, n)).collect();
        let until = Instant::now() + Duration::from_secs(2);
        let mut got = 0;
        while got < want.len() && Instant::now() < until {
            let Some(l) = c.line() else { continue };
            let Some(rest) = l.strip_prefix("STATUS:") else {
                continue;
            };
            let Some((word, path)) = rest.split_once(':') else {
                continue;
            };
            if let Some(n) = want.get(Path::new(path)) {
                got += 1;
                if let Some(s) = state(word) {
                    st.entries.insert(n.clone(), s.into());
                }
            }
        }
        return Some(st);
    }
    None
}

/// Asks the client syncing `paths` to do something with them:
/// "local" (keep on this device), "online" (free up space), "share"
/// (its share dialog), "link" (copy its link to them).
pub fn act(runtime: &Path, what: &str, paths: &[PathBuf]) -> Result<(), String> {
    let cmd = match what {
        "local" => "MAKE_AVAILABLE_LOCALLY",
        "online" => "MAKE_ONLINE_ONLY",
        "share" => "SHARE",
        "link" => "COPY_PRIVATE_LINK",
        _ => return Err(format!("unknown action {what}")),
    };
    let first = paths.first().ok_or("nothing chosen")?;
    for (name, sock, _) in clients(runtime) {
        let Some(mut c) = Conn::open(&sock) else {
            continue;
        };
        if !c.roots().iter().any(|r| first.starts_with(r)) {
            continue;
        }
        // Several paths are separated by the record separator (0x1e).
        let joined: Vec<String> = match what {
            "local" | "online" => vec![paths
                .iter()
                .map(|p| p.to_string_lossy())
                .collect::<Vec<_>>()
                .join("\x1e")],
            _ => vec![first.to_string_lossy().into_owned()],
        };
        for j in joined {
            if !c.send(&format!("{cmd}:{j}")) {
                return Err(format!("{name} stopped answering"));
            }
        }
        return Ok(());
    }
    Err("No sync client is running for this folder".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::net::UnixListener;

    /// A pretend Nextcloud client: says its folder, answers statuses, and
    /// records what it's asked to do.
    fn fake_client(
        runtime: &Path,
        root: &Path,
        states: Vec<(&'static str, &'static str)>,
    ) -> std::sync::mpsc::Receiver<String> {
        std::fs::create_dir_all(runtime.join("Nextcloud")).unwrap();
        let l = UnixListener::bind(runtime.join("Nextcloud/socket")).unwrap();
        let (tx, rx) = std::sync::mpsc::channel();
        let root = root.to_path_buf();
        std::thread::spawn(move || {
            for s in l.incoming().flatten() {
                let mut w = s.try_clone().unwrap();
                writeln!(w, "REGISTER_PATH:{}", root.display()).unwrap();
                writeln!(w, "GET_STRINGS:BEGIN").unwrap(); // other chatter
                for line in BufReader::new(s).lines().map_while(Result::ok) {
                    if let Some(p) = line
                        .split_once(':')
                        .filter(|(k, _)| k.starts_with("RETRIEVE_"))
                        .map(|(_, p)| p.to_string())
                    {
                        let name = Path::new(&p)
                            .file_name()
                            .unwrap()
                            .to_string_lossy()
                            .into_owned();
                        let st = states
                            .iter()
                            .find(|(n, _)| *n == name)
                            .map(|(_, s)| *s)
                            .unwrap_or("NONE");
                        writeln!(w, "STATUS:{st}:{p}").unwrap();
                    } else {
                        tx.send(line).unwrap();
                    }
                }
            }
        });
        rx
    }

    fn dirs(name: &str) -> (PathBuf, PathBuf) {
        let d = std::env::temp_dir().join(format!("wr-cloud-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let (run, root) = (d.join("run"), d.join("Nextcloud"));
        std::fs::create_dir_all(root.join("Photos")).unwrap();
        std::fs::create_dir_all(&run).unwrap();
        (run, root)
    }

    #[test]
    fn badges_for_a_synced_folder() {
        let (run, root) = dirs("status");
        for f in ["report.pdf", "draft.txt", "broken.doc", "notes.md"] {
            std::fs::write(root.join(f), "x").unwrap();
        }
        std::fs::write(root.join("movie.mkv.nextcloud"), "").unwrap();
        let _rx = fake_client(
            &run,
            &root,
            vec![
                ("report.pdf", "OK+SWM"),
                ("draft.txt", "SYNC"),
                ("broken.doc", "ERROR"),
                ("Photos", "OK"),
            ],
        );
        let s = status(&run, &root, 1000).expect("synced");
        assert_eq!(s.client, "Nextcloud");
        let e: Vec<(&str, &str)> = s
            .entries
            .iter()
            .map(|(k, v)| (k.as_str(), v.as_str()))
            .collect();
        assert_eq!(
            e,
            vec![
                ("Photos", "synced"),
                ("broken.doc", "error"),
                ("draft.txt", "syncing"),
                ("movie.mkv.nextcloud", "online"),
                ("report.pdf", "synced")
            ]
        );
        // Outside the synced folder: nothing.
        assert!(status(&run, run.as_path(), 1000).is_none());
        std::fs::remove_dir_all(run.parent().unwrap()).unwrap();
    }

    #[test]
    fn no_client_running() {
        let (run, root) = dirs("none");
        assert!(status(&run, &root, 1000).is_none());
        assert!(act(&run, "local", std::slice::from_ref(&root)).is_err());
        std::fs::remove_dir_all(run.parent().unwrap()).unwrap();
    }

    #[test]
    fn actions_go_to_the_client() {
        let (run, root) = dirs("act");
        let rx = fake_client(&run, &root, vec![]);
        let (a, b) = (root.join("a.mkv"), root.join("Photos"));
        act(&run, "online", &[a.clone(), b.clone()]).unwrap();
        act(&run, "link", std::slice::from_ref(&a)).unwrap();
        let mut got = vec![];
        while let Ok(l) = rx.recv_timeout(Duration::from_secs(2)) {
            got.push(l);
            if got.len() == 2 {
                break;
            }
        }
        assert_eq!(
            got,
            vec![
                format!("MAKE_ONLINE_ONLY:{}\x1e{}", a.display(), b.display()),
                format!("COPY_PRIVATE_LINK:{}", a.display())
            ]
        );
        assert!(act(&run, "explode", &[a]).is_err());
        std::fs::remove_dir_all(run.parent().unwrap()).unwrap();
    }
}
