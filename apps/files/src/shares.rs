//! Network shares: Windows/Samba (SMB), SSH (SFTP), FTP, WebDAV and NFS,
//! through GVfs, the same way GNOME's Files does it. A share is mounted
//! with `gio mount` and then browsed like any folder at its FUSE path
//! ($XDG_RUNTIME_DIR/gvfs/...), so copying, previews and everything else
//! just work.
//!
//! `gio mount` asks for a username, domain and password on its own
//! terminal-style prompts; `mount` answers them. A question (an SSH host
//! key it hasn't seen, a certificate it can't check) is never answered
//! "yes" by itself: it comes back to be shown, and is only accepted when
//! mounting again with `accept`.
//!
//! Saved servers are kept in ~/.config/whiterose/files/servers.json; their
//! passwords are in the keyring, through whiterosed ("share:<address>").

use std::{
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};

/// What's typed into Connect to server, made into an address gio takes.
#[derive(Debug, Clone, PartialEq)]
pub struct Address {
    pub scheme: String,
    pub user: String,
    pub host: String,
    pub port: Option<u16>,
    pub path: String,
}

impl Address {
    pub fn uri(&self) -> String {
        let user = if self.user.is_empty() {
            String::new()
        } else {
            format!("{}@", self.user)
        };
        let port = self.port.map(|p| format!(":{p}")).unwrap_or_default();
        format!("{}://{user}{}{port}{}", self.scheme, self.host, self.path)
    }
    /// "photos on nas", "nas" for a server's top.
    pub fn name(&self) -> String {
        let last = self
            .path
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("");
        if last.is_empty() {
            self.host.clone()
        } else {
            format!("{last} on {}", self.host)
        }
    }
}

const SCHEMES: [&str; 8] = ["smb", "sftp", "ssh", "ftp", "ftps", "dav", "davs", "nfs"];

/// Reads an address as people type them: `smb://nas/photos`,
/// `\\nas\photos` (Windows), `sftp://me@host:2222/srv`, `https://cloud/dav`
/// (WebDAV over TLS), or just `nas/photos` (SMB).
pub fn parse(text: &str) -> Result<Address, String> {
    let t = text.trim();
    if t.is_empty() {
        return Err("Type the server's address".into());
    }
    let (scheme, rest) = if let Some(win) = t.strip_prefix("\\\\") {
        ("smb".to_string(), win.replace('\\', "/"))
    } else if let Some((s, r)) = t.split_once("://") {
        let s = s.to_lowercase();
        let s = match s.as_str() {
            "http" => "dav".to_string(),
            "https" | "webdav" => "davs".to_string(),
            "ssh" => "sftp".to_string(),
            "cifs" => "smb".to_string(),
            _ => s,
        };
        if !SCHEMES.contains(&s.as_str()) {
            return Err(format!("“{s}://” isn't a kind of server Files can open (smb, sftp, ftp, ftps, dav, davs or nfs)"));
        }
        (s, r.to_string())
    } else {
        ("smb".to_string(), t.to_string())
    };
    let (auth_host, path) = match rest.find('/') {
        Some(i) => (rest[..i].to_string(), rest[i..].to_string()),
        None => (rest.clone(), String::new()),
    };
    let (user, host_port) = match auth_host.rsplit_once('@') {
        Some((u, h)) => (u.to_string(), h.to_string()),
        None => (String::new(), auth_host),
    };
    // [v6]:port, host:port, host.
    let (host, port) = if let Some(v6) = host_port.strip_prefix('[') {
        match v6.split_once(']') {
            Some((h, p)) => (format!("[{h}]"), p.strip_prefix(':').map(str::to_string)),
            None => return Err("The address isn't complete".into()),
        }
    } else {
        match host_port.rsplit_once(':') {
            Some((h, p)) => (h.to_string(), Some(p.to_string())),
            None => (host_port, None),
        }
    };
    if host.is_empty() || host.contains(char::is_whitespace) {
        return Err("The address needs a server name".into());
    }
    let port = match port.filter(|p| !p.is_empty()) {
        Some(p) => Some(
            p.parse::<u16>()
                .map_err(|_| format!("“{p}” isn't a port number"))?,
        ),
        None => None,
    };
    let path = path.replace(' ', "%20");
    Ok(Address {
        scheme,
        user,
        host,
        port,
        path: if path == "/" { String::new() } else { path },
    })
}

/// How a mount went.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum Mounted {
    /// Open at this folder.
    Ok {
        path: String,
    },
    /// It wants a password (none was given, or it was wrong).
    Password {
        wrong: bool,
        user: String,
        domain: String,
    },
    /// A question to answer first (mount again with `accept`).
    Question {
        text: String,
    },
    Failed {
        error: String,
    },
}

/// The folder a mounted share is at (its FUSE path), if it's mounted.
pub fn local_path(gio: &str, uri: &str) -> Option<PathBuf> {
    let out = Command::new(gio)
        .args(["info", "-a", "standard::name", uri])
        .env("LC_ALL", "C")
        .stderr(Stdio::null())
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.lines()
        .find_map(|l| l.strip_prefix("local path: "))
        .map(|p| PathBuf::from(p.trim()))
}

/// Mounts `uri` with `gio mount`, answering its prompts. `user`, `domain`
/// and `password` are given when it asks (empty: none to give; with no
/// user at all it tries signing in anonymously). `accept` answers a
/// question with its first choice; otherwise a question comes back.
#[allow(clippy::too_many_arguments)]
pub fn mount(
    gio: &str,
    uri: &str,
    user: &str,
    domain: &str,
    password: &str,
    accept: bool,
    wait: Duration,
    stop: &AtomicBool,
) -> Mounted {
    if let Some(p) = local_path(gio, uri) {
        return Mounted::Ok {
            path: p.to_string_lossy().into_owned(),
        };
    }
    let mut cmd = Command::new(gio);
    cmd.arg("mount");
    let anonymous = user.is_empty() && password.is_empty();
    if anonymous && (uri.starts_with("ftp") || uri.starts_with("smb")) {
        cmd.arg("-a");
    }
    // Its own process group, so stopping it stops anything it started.
    std::os::unix::process::CommandExt::process_group(&mut cmd, 0);
    let child = cmd
        .arg(uri)
        .env("LC_ALL", "C")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn();
    let mut child = match child {
        Ok(c) => c,
        Err(e) => {
            return Mounted::Failed {
                error: format!("Can't run gio (GVfs): {e}"),
            }
        }
    };
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = child.stdout.take().unwrap();
    let mut stderr = child.stderr.take().unwrap();
    // Prompts end without a newline: read as bytes come.
    let (tx, rx) = mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut b = [0u8; 512];
        while let Ok(n) = stdout.read(&mut b) {
            if n == 0 || tx.send(b[..n].to_vec()).is_err() {
                break;
            }
        }
    });
    let err = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = stderr.read_to_string(&mut s);
        s
    });
    let deadline = Instant::now() + wait;
    let mut seen = String::new();
    let mut passwords = 0;
    let mut outcome: Option<Mounted> = None;
    loop {
        if stop.load(Ordering::Relaxed) {
            outcome = Some(Mounted::Failed {
                error: "Cancelled".into(),
            });
            break;
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            outcome = Some(Mounted::Failed {
                error: "The server didn't answer in time".into(),
            });
            break;
        }
        match rx.recv_timeout(left.min(Duration::from_millis(200))) {
            Ok(b) => seen.push_str(&String::from_utf8_lossy(&b)),
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        let tail = seen.rsplit('\n').next().unwrap_or("").to_string();
        let mut answer = |s: &str| {
            let _ = stdin.write_all(format!("{s}\n").as_bytes());
            let _ = stdin.flush();
        };
        if tail.starts_with("User") && tail.ends_with(": ") {
            // "User [default]: " (the default from the address).
            let def = tail
                .split_once('[')
                .and_then(|(_, r)| r.split_once(']'))
                .map(|(d, _)| d)
                .unwrap_or("");
            answer(if user.is_empty() { def } else { user });
            seen.push('\n');
        } else if tail.starts_with("Domain") && tail.ends_with(": ") {
            let def = tail
                .split_once('[')
                .and_then(|(_, r)| r.split_once(']'))
                .map(|(d, _)| d)
                .unwrap_or("");
            answer(if domain.is_empty() { def } else { domain });
            seen.push('\n');
        } else if tail.starts_with("Password") && tail.ends_with(": ") {
            passwords += 1;
            if password.is_empty() || passwords > 1 {
                // None to give, or asked again (the last was wrong).
                outcome = Some(Mounted::Password {
                    wrong: passwords > 1,
                    user: user.into(),
                    domain: domain.into(),
                });
                break;
            }
            answer(password);
            seen.push('\n');
        } else if tail.starts_with("Choice") && tail.ends_with(": ") {
            if accept {
                answer("1");
                seen.push('\n');
            } else {
                // The question: what came before its numbered choices.
                let text: Vec<&str> = seen
                    .lines()
                    .filter(|l| {
                        !l.starts_with('[') && !l.starts_with("Choice") && !l.trim().is_empty()
                    })
                    .collect();
                let text = text
                    .iter()
                    .rev()
                    .take(6)
                    .rev()
                    .cloned()
                    .collect::<Vec<_>>()
                    .join("\n");
                outcome = Some(Mounted::Question { text });
                break;
            }
        }
    }
    drop(stdin);
    if let Some(o) = outcome {
        // Stop it (and anything it started); its output isn't wanted.
        // SAFETY: a negative pid signals the process group we made.
        unsafe {
            libc::kill(-(child.id() as i32), libc::SIGKILL);
        }
        let _ = child.wait();
        return o;
    }
    let status = child.wait();
    let errors = err.join().unwrap_or_default();
    match status {
        Ok(s) if s.success() => match local_path(gio, uri) {
            Some(p) => Mounted::Ok {
                path: p.to_string_lossy().into_owned(),
            },
            None => Mounted::Failed {
                error:
                    "Mounted, but it can't be opened as a folder (GVfs's FUSE part isn't running)"
                        .into(),
            },
        },
        _ => Mounted::Failed {
            error: friendly(&errors),
        },
    }
}

/// gio's error line, without its "gio: <uri>: " start, and what the
/// common ones mean.
fn friendly(stderr: &str) -> String {
    let msg = gio_error(stderr);
    let hint = [
        (
            "Invalid argument",
            "the server's name wasn't found on the network",
        ),
        (
            "Name or service not known",
            "the server's name wasn't found",
        ),
        (
            "Connection refused",
            "the server is there but isn't sharing this way (wrong kind of address or port?)",
        ),
        (
            "No route to host",
            "the server can't be reached from this network",
        ),
        ("Connection timed out", "the server didn't answer"),
        (
            "Permission denied",
            "that username or password isn't allowed in",
        ),
    ]
    .iter()
    .find(|(k, _)| msg.contains(k))
    .map(|(_, h)| *h);
    match hint {
        Some(h) => format!("{msg}: {h}"),
        None => msg,
    }
}

fn gio_error(stderr: &str) -> String {
    let line = stderr
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("Couldn't connect");
    let msg = line.trim_start_matches("gio: ");
    // "smb://nas/: Failed to mount Windows share: Connection refused"
    let msg = match msg.find(": ") {
        Some(i) if msg[..i].contains("://") => &msg[i + 2..],
        _ => msg,
    };
    msg.trim().to_string()
}

/// Unmounts a share (files open on it are closed by GVfs).
pub fn unmount(gio: &str, uri: &str) -> Result<(), String> {
    let out = Command::new(gio)
        .args(["mount", "-u", uri])
        .env("LC_ALL", "C")
        .output()
        .map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(friendly(&String::from_utf8_lossy(&out.stderr)))
    }
}

/// Network shares mounted now (any app's): (address, name), from one
/// `gio mount -l` ("Mount(0): photos on nas -> smb://nas/photos/").
pub fn mounted(gio: &str) -> Vec<(String, String)> {
    let Ok(out) = Command::new(gio)
        .args(["mount", "-l"])
        .env("LC_ALL", "C")
        .stderr(Stdio::null())
        .output()
    else {
        return vec![];
    };
    parse_mounts(&String::from_utf8_lossy(&out.stdout))
}

fn parse_mounts(text: &str) -> Vec<(String, String)> {
    text.lines()
        .filter_map(|l| {
            l.trim()
                .strip_prefix("Mount(")?
                .split_once("): ")
                .map(|(_, r)| r.to_string())
        })
        .filter_map(|r| {
            let (name, uri) = r.rsplit_once(" -> ")?;
            let scheme = uri.split("://").next()?;
            // (gio names a Nextcloud "jack@storage.example.org".)
            let name = if is_nextcloud(uri) { "Nextcloud" } else { name };
            SCHEMES
                .contains(&scheme)
                .then(|| (uri.trim_end_matches('/').to_string(), name.to_string()))
        })
        .collect()
}

/// A WebDAV address that's a Nextcloud (its files are at /remote.php/).
pub fn is_nextcloud(uri: &str) -> bool {
    uri.starts_with("dav") && uri.contains("/remote.php/")
}

/// An online account's drive (GNOME Online Accounts: Nextcloud, OneDrive,
/// ...), mounted with the account's own sign-in.
#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Account {
    /// The volume's id in GOA's volume monitor (to mount it).
    pub id: String,
    pub name: String,
    /// Where it is (davs://..., onedrive://...).
    pub uri: String,
}

const GOA: [&str; 6] = [
    "--session",
    "--dest",
    "org.gtk.vfs.GoaVolumeMonitor",
    "--object-path",
    "/org/gtk/Private/RemoteVolumeMonitor",
    "--method",
];

/// The online accounts' drives (none without GNOME Online Accounts).
pub fn accounts() -> Vec<Account> {
    let Ok(out) = Command::new("gdbus")
        .args(["call", "--timeout", "5"])
        .args(GOA)
        .arg("org.gtk.Private.RemoteVolumeMonitor.List")
        .stderr(Stdio::null())
        .output()
    else {
        return vec![];
    };
    parse_accounts(&String::from_utf8_lossy(&out.stdout))
}

/// Mounts an account's drive: GOA signs in with the account.
pub fn mount_account(a: &Account) -> Mounted {
    if let Some(p) = local_path("gio", &a.uri) {
        return Mounted::Ok {
            path: p.to_string_lossy().into_owned(),
        };
    }
    let out = Command::new("gdbus")
        .args(["call", "--timeout", "60"])
        .args(GOA)
        .arg("org.gtk.Private.RemoteVolumeMonitor.VolumeMount")
        .args([a.id.as_str(), "", "0", ""])
        .output();
    match out {
        Ok(o) if o.status.success() => match local_path("gio", &a.uri) {
            Some(p) => Mounted::Ok {
                path: p.to_string_lossy().into_owned(),
            },
            None => Mounted::Failed {
                error:
                    "Signed in, but it can't be opened as a folder (GVfs's FUSE part isn't running)"
                        .into(),
            },
        },
        Ok(o) => Mounted::Failed {
            error: String::from_utf8_lossy(&o.stderr)
                .lines()
                .last()
                .unwrap_or("Couldn't sign in")
                .trim_start_matches("Error: ")
                .to_string(),
        },
        Err(e) => Mounted::Failed {
            error: format!("Can't run gdbus: {e}"),
        },
    }
}

/// Splits GVariant text at commas that aren't inside brackets or quotes.
fn split_top(s: &str) -> Vec<&str> {
    let (mut out, mut depth, mut start, mut quote, mut esc) =
        (vec![], 0i32, 0usize, None::<char>, false);
    for (i, c) in s.char_indices() {
        if let Some(q) = quote {
            if esc {
                esc = false;
            } else if c == '\\' {
                esc = true;
            } else if c == q {
                quote = None;
            }
            continue;
        }
        match c {
            '\'' | '"' => quote = Some(c),
            '(' | '[' | '{' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            ',' if depth == 0 => {
                out.push(s[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    if !s[start..].trim().is_empty() {
        out.push(s[start..].trim());
    }
    out
}

fn unwrap(s: &str, open: char, close: char) -> Option<&str> {
    let s = s.trim();
    // A typed empty array: "@a(ss) []".
    let s = match s.find(open) {
        Some(i) if s.starts_with('@') => &s[i..],
        _ => s,
    };
    s.strip_prefix(open)?.strip_suffix(close)
}

fn unquote(s: &str) -> String {
    let s = s.trim();
    let s = s
        .strip_prefix('\'')
        .and_then(|x| x.strip_suffix('\''))
        .unwrap_or(s);
    s.replace("\\'", "'")
}

fn parse_accounts(text: &str) -> Vec<Account> {
    // (drives, volumes, mounts)
    let Some(all) = unwrap(text, '(', ')') else {
        return vec![];
    };
    let parts = split_top(all);
    let Some(volumes) = parts.get(1).and_then(|v| unwrap(v, '[', ']')) else {
        return vec![];
    };
    let mut out: Vec<Account> = split_top(volumes)
        .into_iter()
        .filter_map(|t| {
            let f = split_top(unwrap(t, '(', ')')?);
            // (id, name, icon, symbolic icon, uuid, activation root, ...)
            let (id, name, uri) = (unquote(f.first()?), unquote(f.get(1)?), unquote(f.get(5)?));
            (!uri.is_empty()).then(|| Account {
                id,
                name,
                uri: uri.trim_end_matches('/').to_string(),
            })
        })
        .collect();
    // A Nextcloud account is named "jack@storage.example.org" by GOA:
    // "Nextcloud" (with its server when there's more than one).
    let nextcloud = |a: &Account| is_nextcloud(&a.uri);
    let several = out.iter().filter(|a| nextcloud(a)).count() > 1;
    for a in out.iter_mut().filter(|a| nextcloud(a)) {
        let host = a
            .uri
            .split("://")
            .nth(1)
            .unwrap_or("")
            .split('/')
            .next()
            .unwrap_or("");
        let host = host
            .rsplit('@')
            .next()
            .unwrap_or(host)
            .split(':')
            .next()
            .unwrap_or(host);
        a.name = if several {
            format!("Nextcloud ({host})")
        } else {
            "Nextcloud".into()
        };
    }
    out
}

/// The hosts named in an SSH config ("Host build nas", not patterns like
/// "*.example.com"): `ssh` (and so SFTP here) knows their user, port and
/// key, so `sftp://build` is all it takes.
pub fn ssh_hosts(config: &str) -> Vec<String> {
    let mut out: Vec<String> = vec![];
    for line in config.lines() {
        let l = line.trim();
        let Some((k, v)) = l.split_once(|c: char| c.is_whitespace() || c == '=') else {
            continue;
        };
        if !k.eq_ignore_ascii_case("host") {
            continue;
        }
        for h in v.split_whitespace() {
            let h = h.trim_matches('"');
            if !h.contains(['*', '?', '!']) && !h.is_empty() && !out.iter().any(|x| x == h) {
                out.push(h.to_string());
            }
        }
    }
    out
}

/// A server kept in the sidebar.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Server {
    pub name: String,
    pub uri: String,
    #[serde(default)]
    pub user: String,
    #[serde(default)]
    pub domain: String,
    /// Its password is in the keyring.
    #[serde(default)]
    pub remembered: bool,
}

pub fn servers_file() -> PathBuf {
    crate::places::config_dir().join("whiterose/files/servers.json")
}

pub fn load(file: &Path) -> Vec<Server> {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save(file: &Path, servers: &[Server]) -> Result<(), String> {
    if let Some(d) = file.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = file.with_extension("json.tmp");
    std::fs::write(
        &tmp,
        serde_json::to_string_pretty(servers).unwrap_or_default(),
    )
    .map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, file).map_err(|e| e.to_string())
}

/// Adds or updates (by address) a saved server.
pub fn remember(list: &mut Vec<Server>, s: Server) {
    match list.iter_mut().find(|x| x.uri == s.uri) {
        Some(x) => *x = s,
        None => list.push(s),
    }
}

/// The keyring id of a server's password.
pub fn secret_id(uri: &str, user: &str) -> String {
    format!("share:{uri}#{user}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_addresses_as_typed() {
        let a = parse("smb://nas/photos").unwrap();
        assert_eq!(
            (a.scheme.as_str(), a.host.as_str(), a.path.as_str()),
            ("smb", "nas", "/photos")
        );
        assert_eq!(a.uri(), "smb://nas/photos");
        assert_eq!(a.name(), "photos on nas");
        assert_eq!(
            parse(r"\\nas\photos\2026").unwrap().uri(),
            "smb://nas/photos/2026"
        );
        assert_eq!(parse("nas/music").unwrap().uri(), "smb://nas/music");
        let s = parse("ssh://jack@build.local:2222/srv/www").unwrap();
        assert_eq!(
            (s.scheme.as_str(), s.user.as_str(), s.port),
            ("sftp", "jack", Some(2222))
        );
        assert_eq!(s.uri(), "sftp://jack@build.local:2222/srv/www");
        assert_eq!(
            parse("https://cloud.example.com/remote.php/dav")
                .unwrap()
                .uri(),
            "davs://cloud.example.com/remote.php/dav"
        );
        assert_eq!(
            parse("ftp://[fe80::1]:21/").unwrap().uri(),
            "ftp://[fe80::1]:21"
        );
        assert_eq!(parse("sftp://host").unwrap().name(), "host");
        assert_eq!(
            parse("smb://nas/My Photos").unwrap().uri(),
            "smb://nas/My%20Photos"
        );
        for bad in [
            "",
            "gopher://x",
            "smb://",
            "smb://nas:port/x",
            "smb://my nas/x",
        ] {
            assert!(parse(bad).is_err(), "{bad:?} accepted");
        }
    }

    #[test]
    fn lists_network_mounts_only() {
        let text = "Drive(0): WDSN740\n  Type: GProxyDrive\nMount(0): photos on nas -> smb://nas/photos/\n  Type: GDaemonMount\nMount(1): Boot -> file:///boot\nMount(2): jack on build -> sftp://jack@build/\nMount(3): jack@storage.bluebeard.site -> davs://jack@storage.bluebeard.site/remote.php/webdav/\n";
        assert_eq!(
            parse_mounts(text),
            vec![
                ("smb://nas/photos".into(), "photos on nas".into()),
                ("sftp://jack@build".into(), "jack on build".into()),
                (
                    "davs://jack@storage.bluebeard.site/remote.php/webdav".into(),
                    "Nextcloud".into()
                )
            ]
        );
    }

    #[test]
    fn reads_online_accounts() {
        // As gdbus prints it (from a machine with a Nextcloud account).
        let text = "(@a(ssssbbbbbbbbuasa{ss}sa{sv}) [], [('0x7387680041c0', 'jack@storage.bluebeard.site', '. GThemedIcon network-server network', '. GThemedIcon network-server-symbolic', 'davs://jack@storage.bluebeard.site:443/remote.php/webdav/', 'davs://jack@storage.bluebeard.site/remote.php/webdav', true, false, '', '', {'class': 'network', 'uuid': 'davs://x'}, '', {'always-call-mount': <true>}), ('0x2', 'Jack\\'s OneDrive', '', '', '', 'onedrive://jack/', true, false, '', '', {}, '', {})], @a(ssssssbsassa{sv}) [])\n";
        assert_eq!(
            parse_accounts(text),
            vec![
                Account {
                    id: "0x7387680041c0".into(),
                    name: "Nextcloud".into(),
                    uri: "davs://jack@storage.bluebeard.site/remote.php/webdav".into()
                },
                Account {
                    id: "0x2".into(),
                    name: "Jack's OneDrive".into(),
                    uri: "onedrive://jack".into()
                },
            ]
        );
        assert!(parse_accounts("(@a(s) [], @a(s) [], @a(s) [])").is_empty());
        assert!(parse_accounts("Error: no such name").is_empty());
    }

    #[test]
    fn hosts_from_the_ssh_config() {
        let cfg = "# mine\nHost build nas\n  User jack\nHost *.corp !bad\nhost=pi \"web\"\nHostName ignored.example\nHost build\n";
        assert_eq!(ssh_hosts(cfg), ["build", "nas", "pi", "web"]);
    }

    #[test]
    fn friendly_errors() {
        assert_eq!(
            friendly("gio: smb://nas/: Failed to mount Windows share: Connection refused\n"),
            "Failed to mount Windows share: Connection refused: the server is there but isn't sharing this way (wrong kind of address or port?)"
        );
        assert_eq!(friendly("gio: sftp://x/: Weird\n"), "Weird");
        assert_eq!(friendly(""), "Couldn't connect");
    }

    #[test]
    fn keeps_saved_servers() {
        let d = std::env::temp_dir().join(format!("wr-shares-{}", std::process::id()));
        let f = d.join("servers.json");
        let mut l = vec![];
        remember(
            &mut l,
            Server {
                name: "nas".into(),
                uri: "smb://nas".into(),
                user: "jack".into(),
                domain: String::new(),
                remembered: true,
            },
        );
        remember(
            &mut l,
            Server {
                name: "NAS".into(),
                uri: "smb://nas".into(),
                user: "jack".into(),
                domain: String::new(),
                remembered: false,
            },
        );
        assert_eq!(l.len(), 1);
        save(&f, &l).unwrap();
        assert_eq!(load(&f), l);
        assert_eq!(secret_id("smb://nas", "jack"), "share:smb://nas#jack");
        let _ = std::fs::remove_dir_all(d);
    }

    /// A stand-in for gio that asks what gio asks, and records the answers.
    fn fake_gio(dir: &Path, script: &str) -> String {
        let p = dir.join("gio");
        std::fs::write(&p, format!("#!/bin/sh\n{script}")).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn answers_the_prompts() {
        let d = std::env::temp_dir().join(format!("wr-gio-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let log = d.join("answers");
        let mounted = d.join("mounted");
        // `gio info` says where once mounted; `gio mount` asks, checks, mounts.
        let script = format!(
            r#"if [ "$1" = info ]; then [ -f {m} ] && echo "local path: /run/user/1/gvfs/smb-share:server=nas"; exit 0; fi
echo "Authentication Required"
echo "Enter user and password for share “photos” on “nas”:"
printf "User [guest]: "; read u
printf "Domain [WORKGROUP]: "; read dm
while true; do
  printf "Password: "; read p || exit 2
  echo "$u $dm $p" >> {l}
  [ "$p" = right ] && break
done
touch {m}
"#,
            m = mounted.display(),
            l = log.display()
        );
        let gio = fake_gio(&d, &script);
        // No password to give: it says so.
        let r = mount(
            &gio,
            "smb://nas/photos",
            "jack",
            "",
            "",
            false,
            Duration::from_secs(5),
            &AtomicBool::new(false),
        );
        assert_eq!(
            r,
            Mounted::Password {
                wrong: false,
                user: "jack".into(),
                domain: "".into()
            }
        );
        // Wrong, then right.
        let r = mount(
            &gio,
            "smb://nas/photos",
            "jack",
            "HOME",
            "wrong",
            false,
            Duration::from_secs(5),
            &AtomicBool::new(false),
        );
        assert_eq!(
            r,
            Mounted::Password {
                wrong: true,
                user: "jack".into(),
                domain: "HOME".into()
            }
        );
        let r = mount(
            &gio,
            "smb://nas/photos",
            "jack",
            "",
            "right",
            false,
            Duration::from_secs(5),
            &AtomicBool::new(false),
        );
        assert_eq!(
            r,
            Mounted::Ok {
                path: "/run/user/1/gvfs/smb-share:server=nas".into()
            }
        );
        // The domain left empty takes gio's default.
        let answers = std::fs::read_to_string(&log).unwrap();
        assert!(
            answers.contains("jack HOME wrong") && answers.contains("jack WORKGROUP right"),
            "{answers}"
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn questions_come_back_unless_accepted() {
        let d = std::env::temp_dir().join(format!("wr-gioq-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let mounted = d.join("mounted");
        let script = format!(
            r#"if [ "$1" = info ]; then [ -f {m} ] && echo "local path: /x"; exit 0; fi
echo "The identity of the remote computer (build) is unknown."
echo "The fingerprint is SHA256:abc."
echo "[1] Log In Anyway"
echo "[2] Cancel Login"
printf "Choice: "; read c
[ "$c" = 1 ] || {{ echo "gio: sftp://build/: Login dialog cancelled" >&2; exit 2; }}
touch {m}
"#,
            m = mounted.display()
        );
        let gio = fake_gio(&d, &script);
        match mount(
            &gio,
            "sftp://build",
            "",
            "",
            "",
            false,
            Duration::from_secs(5),
            &AtomicBool::new(false),
        ) {
            Mounted::Question { text } => assert!(
                text.contains("unknown") && text.contains("SHA256:abc") && !text.contains("[1]"),
                "{text}"
            ),
            other => panic!("{other:?}"),
        }
        assert!(!mounted.exists(), "a question was answered by itself");
        assert_eq!(
            mount(
                &gio,
                "sftp://build",
                "",
                "",
                "",
                true,
                Duration::from_secs(5),
                &AtomicBool::new(false)
            ),
            Mounted::Ok { path: "/x".into() }
        );
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn cancelling_stops_it() {
        let d = std::env::temp_dir().join(format!("wr-gioc-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let gio = fake_gio(&d, "[ \"$1\" = info ] && exit 0\nsleep 30\n");
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let s2 = stop.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            s2.store(true, Ordering::Relaxed);
        });
        let t = Instant::now();
        assert_eq!(
            mount(
                &gio,
                "smb://slow",
                "",
                "",
                "",
                false,
                Duration::from_secs(20),
                &stop
            ),
            Mounted::Failed {
                error: "Cancelled".into()
            }
        );
        assert!(t.elapsed() < Duration::from_secs(3));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn failures_say_why() {
        let d = std::env::temp_dir().join(format!("wr-giof-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let gio = fake_gio(&d, "[ \"$1\" = info ] && exit 0\necho 'gio: smb://nas/: Failed to mount Windows share: Connection refused' >&2\nexit 2\n");
        assert_eq!(
            mount(&gio, "smb://nas", "", "", "", false, Duration::from_secs(5), &AtomicBool::new(false)),
            Mounted::Failed { error: "Failed to mount Windows share: Connection refused: the server is there but isn't sharing this way (wrong kind of address or port?)".into() }
        );
        std::fs::remove_dir_all(&d).unwrap();
    }
}
