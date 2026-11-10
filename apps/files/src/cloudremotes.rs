//! Cloud drives without a desktop client on Linux (Google Drive, OneDrive,
//! Dropbox, iCloud Drive, S3-compatible storage), through rclone, opened
//! as folders with `rclone mount`.
//!
//! Adding an account drives rclone's own setup in its non-interactive
//! mode: each step, rclone says what it needs next as JSON (a question,
//! its help, choices or a default); Files answers the ones that are
//! always the same here (sign in with the web browser on this computer)
//! and shows you the rest (an iCloud two-factor code, S3 keys...), then
//! carries on with `--continue`. Signing in with a browser happens in your
//! browser; rclone keeps the account in its own config (~/.config/rclone).

use std::{
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant},
};

use serde::Serialize;
use serde_json::Value;

/// The kinds of account offered: (rclone type, name shown).
pub const PROVIDERS: [(&str, &str); 6] = [
    ("drive", "Google Drive"),
    ("onedrive", "OneDrive"),
    ("dropbox", "Dropbox"),
    ("iclouddrive", "iCloud Drive"),
    (
        "s3",
        "S3-compatible storage (Amazon S3, Backblaze B2, Wasabi, MinIO…)",
    ),
    ("pcloud", "pCloud"),
];

pub fn provider_name(kind: &str) -> &str {
    PROVIDERS
        .iter()
        .find(|(k, _)| *k == kind)
        .map(|(_, n)| *n)
        .unwrap_or(kind)
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct Remote {
    pub name: String,
    pub kind: String,
}

/// `rclone listremotes --long`: "gdrive:   drive".
pub fn parse_remotes(text: &str) -> Vec<Remote> {
    text.lines()
        .filter_map(|l| {
            let (n, k) = l.split_once(':')?;
            Some(Remote {
                name: n.trim().to_string(),
                kind: k.trim().to_string(),
            })
        })
        .filter(|r| !r.name.is_empty())
        .collect()
}

pub fn remotes(rclone: &str) -> Vec<Remote> {
    Command::new(rclone)
        .args(["listremotes", "--long"])
        .stderr(Stdio::null())
        .output()
        .map(|o| parse_remotes(&String::from_utf8_lossy(&o.stdout)))
        .unwrap_or_default()
}

/// A name rclone takes for an account (and that's a fine folder name).
pub fn check_name(name: &str) -> Result<&str, String> {
    let n = name.trim();
    if n.is_empty() {
        return Err("Give it a name".into());
    }
    if n.len() > 60
        || n.starts_with('-')
        || !n.chars().all(|c| c.is_alphanumeric() || " _-.".contains(c))
    {
        return Err("Use letters, numbers, spaces, - _ and . in the name".into());
    }
    Ok(n)
}

/// Where a step of setting an account up got to.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum Step {
    Done,
    /// rclone wants this answered (then `next` with `state`).
    Ask {
        state: String,
        name: String,
        help: String,
        default: String,
        /// [(value, what it means)]
        choices: Vec<(String, String)>,
        password: bool,
        required: bool,
    },
    Failed {
        error: String,
    },
}

/// Answered by Files, never asked: rclone signs in through the web browser
/// on this computer.
fn automatic(option: &str) -> Option<&'static str> {
    match option {
        "config_is_local" => Some("true"),
        _ => None,
    }
}

fn text(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Reads rclone's answer to a step (its JSON on stdout).
pub fn parse_step(out: &str) -> Result<(String, Option<Value>, String), String> {
    let start = out.find('{').ok_or("rclone didn't say what's next")?;
    let v: Value = serde_json::from_str(&out[start..])
        .map_err(|e| format!("rclone's answer wasn't readable: {e}"))?;
    let state = text(&v["State"]);
    let err = text(&v["Error"]);
    let opt = v.get("Option").filter(|o| !o.is_null()).cloned();
    Ok((state, opt, err))
}

/// Runs one rclone setup command to its next question (or the end),
/// telling `note` what it says along the way (a sign-in link).
fn run(
    rclone: &str,
    args: &[String],
    wait: Duration,
    stop: &AtomicBool,
    note: &mut dyn FnMut(String),
) -> Result<String, String> {
    let mut child = Command::new(rclone)
        .args(args)
        .arg("--non-interactive")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Can't run rclone: {e}"))?;
    let stdout = child.stdout.take().unwrap();
    let out = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = std::io::Read::read_to_string(&mut BufReader::new(stdout), &mut s);
        s
    });
    let stderr = child.stderr.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        for l in BufReader::new(stderr).lines().map_while(Result::ok) {
            if tx.send(l).is_err() {
                break;
            }
        }
    });
    let until = Instant::now() + wait;
    let mut last_err = String::new();
    loop {
        while let Ok(l) = rx.try_recv() {
            // "If your browser doesn't open automatically go to the following link: http://127.0.0.1:53682/auth?state=…"
            if let Some(i) = l.find("http://127.0.0.1:") {
                note(l[i..].split_whitespace().next().unwrap_or("").to_string());
            }
            if l.contains("ERROR") || l.contains("Failed") {
                last_err = l.split_once(": ").map(|(_, r)| r).unwrap_or(&l).to_string();
            }
        }
        match child.try_wait() {
            Ok(Some(status)) => {
                let s = out.join().unwrap_or_default();
                while let Ok(l) = rx.try_recv() {
                    if l.contains("ERROR") || l.contains("Failed") {
                        last_err = l.split_once(": ").map(|(_, r)| r).unwrap_or(&l).to_string();
                    }
                }
                if !status.success() && s.trim().is_empty() {
                    return Err(if last_err.is_empty() {
                        "rclone couldn't set it up".into()
                    } else {
                        last_err
                    });
                }
                return Ok(s);
            }
            Ok(None) => {}
            Err(e) => return Err(e.to_string()),
        }
        if stop.load(Ordering::Relaxed) || Instant::now() > until {
            let _ = child.kill();
            let _ = child.wait();
            return Err(if stop.load(Ordering::Relaxed) {
                "Cancelled".into()
            } else {
                "It took too long (the sign-in wasn't finished)".into()
            });
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

/// Starts (`state` None: a new account of `kind` with `params`) or carries
/// on (`state` and `answer`) setting up account `name`, answering what
/// Files answers itself, until there's a question for you or it's done.
#[allow(clippy::too_many_arguments)]
pub fn setup(
    rclone: &str,
    name: &str,
    kind: &str,
    params: &[(String, String)],
    state: Option<(&str, &str)>,
    stop: &AtomicBool,
    note: &mut dyn FnMut(String),
) -> Step {
    let name = match check_name(name) {
        Ok(n) => n.to_string(),
        Err(e) => return Step::Failed { error: e },
    };
    let mut args: Vec<String> = match state {
        None => {
            if !PROVIDERS.iter().any(|(k, _)| *k == kind) {
                return Step::Failed {
                    error: format!("{kind} accounts aren't offered"),
                };
            }
            let mut a = vec!["config".into(), "create".into(), name.clone(), kind.into()];
            a.extend(params.iter().map(|(k, v)| format!("{k}={v}")));
            a
        }
        Some((st, ans)) => continue_args(&name, st, ans),
    };
    // A few rounds: Files' own answers, then a question for you.
    for _ in 0..12 {
        let out = match run(rclone, &args, Duration::from_secs(600), stop, note) {
            Ok(o) => o,
            Err(e) => return Step::Failed { error: e },
        };
        let (st, opt, err) = match parse_step(&out) {
            Ok(x) => x,
            Err(e) => return Step::Failed { error: e },
        };
        let Some(opt) = opt else {
            return if err.is_empty() {
                Step::Done
            } else {
                Step::Failed { error: err }
            };
        };
        let oname = text(&opt["Name"]);
        if let Some(a) = automatic(&oname) {
            args = continue_args(&name, &st, a);
            continue;
        }
        let choices = opt["Examples"]
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|e| (text(&e["Value"]), text(&e["Help"])))
                    .collect()
            })
            .unwrap_or_default();
        let help = text(&opt["Help"]);
        return Step::Ask {
            state: st,
            name: oname,
            help: if err.is_empty() {
                help
            } else {
                format!("{err}\n\n{help}")
            },
            default: text(&opt["Default"]),
            choices,
            password: opt["IsPassword"].as_bool().unwrap_or(false),
            required: opt["Required"].as_bool().unwrap_or(false),
        };
    }
    Step::Failed {
        error: "rclone kept asking: set it up with `rclone config` instead".into(),
    }
}

fn continue_args(name: &str, state: &str, answer: &str) -> Vec<String> {
    vec![
        "config".into(),
        "update".into(),
        name.into(),
        "--continue".into(),
        "--state".into(),
        state.into(),
        "--result".into(),
        answer.into(),
    ]
}

/// Where an account opens: $XDG_RUNTIME_DIR/whiterose/cloud/<name>.
pub fn mount_dir(runtime: &Path, name: &str) -> PathBuf {
    runtime.join("whiterose/cloud").join(name)
}

/// Whether something is mounted at `dir` (from /proc/self/mountinfo).
pub fn is_mounted(mountinfo: &str, dir: &Path) -> bool {
    let want = dir.to_string_lossy().replace(' ', "\\040");
    mountinfo
        .lines()
        .any(|l| l.split(' ').nth(4) == Some(want.as_str()))
}

pub fn mounted(dir: &Path) -> bool {
    std::fs::read_to_string("/proc/self/mountinfo")
        .map(|m| is_mounted(&m, dir))
        .unwrap_or(false)
}

/// Opens account `name` at `dir` (rclone keeps running, in the
/// background, until it's unmounted). Files are cached on disk as they're
/// used, so opening, saving and editing work as on a local drive.
pub fn mount(rclone: &str, name: &str, dir: &Path) -> Result<(), String> {
    if mounted(dir) {
        return Ok(());
    }
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let out = Command::new(rclone)
        .args(["mount", &format!("{name}:")])
        .arg(dir)
        .args([
            "--vfs-cache-mode",
            "full",
            "--daemon",
            "--daemon-wait",
            "30s",
        ])
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("Can't run rclone: {e}"))?;
    if out.status.success() && mounted(dir) {
        return Ok(());
    }
    // Nothing mounted: the empty folder made for it goes too.
    let _ = std::fs::remove_dir(dir);
    Err(mount_error(&String::from_utf8_lossy(&out.stderr)))
}

/// rclone's reason, said plainly.
fn mount_error(stderr: &str) -> String {
    if stderr.contains("fusermount") || stderr.contains("/dev/fuse") {
        return "FUSE isn't installed, so it can't open as a folder: install fuse3 (Settings › Apps installs it with rclone)".into();
    }
    let line = stderr
        .lines()
        .rev()
        .find(|l| !l.trim().is_empty())
        .unwrap_or("rclone couldn't open it");
    // "2026/11/10 18:00:00 CRITICAL: Fatal error: <why>"
    let why = line.rsplit(": ").next().unwrap_or(line).trim();
    if why.contains("token") || why.contains("401") || why.contains("invalid_grant") {
        return "The sign-in has expired: remove the account and add it again".into();
    }
    why.to_string()
}

pub fn unmount(dir: &Path) -> Result<(), String> {
    for tool in ["fusermount3", "fusermount"] {
        if let Ok(o) = Command::new(tool).arg("-u").arg(dir).output() {
            if o.status.success() {
                let _ = std::fs::remove_dir(dir);
                return Ok(());
            }
            let e = String::from_utf8_lossy(&o.stderr).trim().to_string();
            if !e.is_empty() {
                return Err(if e.contains("busy") {
                    "Something still has files open there".into()
                } else {
                    e
                });
            }
        }
    }
    Err("Can't unmount (fusermount isn't installed)".into())
}

/// Takes an account out of rclone's config (the cloud keeps the files).
pub fn remove(rclone: &str, name: &str) -> Result<(), String> {
    let o = Command::new(rclone)
        .args(["config", "delete", name])
        .output()
        .map_err(|e| e.to_string())?;
    if o.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&o.stderr).trim().to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_remotes_and_names() {
        assert_eq!(
            parse_remotes("gdrive:     drive\nMy Box: s3\n\n"),
            vec![
                Remote {
                    name: "gdrive".into(),
                    kind: "drive".into()
                },
                Remote {
                    name: "My Box".into(),
                    kind: "s3".into()
                }
            ]
        );
        assert!(check_name("Work Drive").is_ok());
        for bad in ["", "-x", "a/b", "a:b", &"x".repeat(61)] {
            assert!(check_name(bad).is_err(), "{bad:?}");
        }
        assert_eq!(provider_name("iclouddrive"), "iCloud Drive");
    }

    #[test]
    fn mount_errors_said_plainly() {
        assert!(mount_error("2026/11/10 CRITICAL: Fatal error: failed to mount FUSE fs: fusermount: exec: \"fusermount3\": executable file not found in $PATH").starts_with("FUSE isn't installed"));
        assert!(mount_error(
            "CRITICAL: Fatal error: couldn't refresh token: oauth2: invalid_grant"
        )
        .starts_with("The sign-in has expired"));
        assert_eq!(
            mount_error("ERROR : directory not found"),
            "directory not found"
        );
    }

    #[test]
    fn knows_whats_mounted() {
        let info = "36 25 0:32 / /run/user/1000/whiterose/cloud/My\\040Drive rw,nosuid shared:1 - fuse.rclone gdrive: rw\n22 1 8:2 / / rw - ext4 /dev/sda2 rw\n";
        assert!(is_mounted(
            info,
            Path::new("/run/user/1000/whiterose/cloud/My Drive")
        ));
        assert!(!is_mounted(
            info,
            Path::new("/run/user/1000/whiterose/cloud/Other")
        ));
        assert_eq!(
            mount_dir(Path::new("/run/user/1"), "gdrive"),
            PathBuf::from("/run/user/1/whiterose/cloud/gdrive")
        );
    }

    /// A stand-in rclone: config create asks whether to use a browser
    /// (answered by Files), then for a two-factor code (asked of you),
    /// then is done; it says a sign-in link on the way.
    fn fake_rclone(dir: &Path) -> String {
        let p = dir.join("rclone");
        let log = dir.join("calls");
        std::fs::write(
            &p,
            format!(
                r#"#!/bin/sh
echo "$*" >> {log}
case "$*" in
  "config create"*)
    echo '{{"State":"*local","Option":{{"Name":"config_is_local","Help":"Use a browser?","Default":true,"Examples":[{{"Value":"true","Help":"Yes"}}],"IsPassword":false,"Required":false}},"Error":"","Result":""}}' ;;
  *"--state *local --result true"*)
    echo "NOTICE: If your browser doesn't open automatically go to the following link: http://127.0.0.1:53682/auth?state=abc" >&2
    echo '{{"State":"*2fa","Option":{{"Name":"config_2fa","Help":"Two-factor code","Default":"","Examples":null,"IsPassword":false,"Required":true}},"Error":"","Result":""}}' ;;
  *"--state *2fa --result 123456"*)
    echo '{{"State":"","Option":null,"Error":"","Result":""}}' ;;
  *"--state *2fa"*)
    echo '{{"State":"*2fa","Option":{{"Name":"config_2fa","Help":"Two-factor code","Default":"","IsPassword":false,"Required":true}},"Error":"That code was wrong","Result":""}}' ;;
  *) echo "unexpected" >&2; exit 1 ;;
esac
"#,
                log = log.display()
            ),
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p.to_string_lossy().into_owned()
    }

    #[test]
    fn sets_up_an_account_step_by_step() {
        let d = std::env::temp_dir().join(format!("wr-rclone-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let rc = fake_rclone(&d);
        let stop = AtomicBool::new(false);
        let mut notes = vec![];
        // Files answers "use a browser" itself; the code is asked.
        let s = setup(
            &rc,
            "icloud",
            "iclouddrive",
            &[("apple_id".into(), "me@example.com".into())],
            None,
            &stop,
            &mut |n| notes.push(n),
        );
        let Step::Ask {
            state,
            name,
            required,
            ..
        } = s
        else {
            panic!("{s:?}")
        };
        assert_eq!(
            (state.as_str(), name.as_str(), required),
            ("*2fa", "config_2fa", true)
        );
        assert_eq!(notes, ["http://127.0.0.1:53682/auth?state=abc"]);
        // A wrong answer: asked again, saying why.
        match setup(
            &rc,
            "icloud",
            "",
            &[],
            Some(("*2fa", "000000")),
            &stop,
            &mut |_| {},
        ) {
            Step::Ask { help, .. } => assert!(help.starts_with("That code was wrong"), "{help}"),
            other => panic!("{other:?}"),
        }
        assert_eq!(
            setup(
                &rc,
                "icloud",
                "",
                &[],
                Some(("*2fa", "123456")),
                &stop,
                &mut |_| {}
            ),
            Step::Done
        );
        let calls = std::fs::read_to_string(d.join("calls")).unwrap();
        assert!(
            calls.starts_with(
                "config create icloud iclouddrive apple_id=me@example.com --non-interactive"
            ),
            "{calls}"
        );
        // Not an offered kind, or a bad name: refused before rclone runs.
        assert!(matches!(
            setup(&rc, "x", "ftp", &[], None, &stop, &mut |_| {}),
            Step::Failed { .. }
        ));
        assert!(matches!(
            setup(&rc, "a/b", "drive", &[], None, &stop, &mut |_| {}),
            Step::Failed { .. }
        ));
        std::fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn cancelling_stops_a_sign_in() {
        let d = std::env::temp_dir().join(format!("wr-rclone-stop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        let p = d.join("rclone");
        std::fs::write(&p, "#!/bin/sh\nexec sleep 30\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        let stop = std::sync::Arc::new(AtomicBool::new(false));
        let s2 = stop.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(300));
            s2.store(true, Ordering::Relaxed);
        });
        let t = Instant::now();
        let r = setup(
            &p.to_string_lossy(),
            "g",
            "drive",
            &[],
            None,
            &stop,
            &mut |_| {},
        );
        assert_eq!(
            r,
            Step::Failed {
                error: "Cancelled".into()
            }
        );
        assert!(t.elapsed() < Duration::from_secs(3));
        std::fs::remove_dir_all(&d).unwrap();
    }
}
