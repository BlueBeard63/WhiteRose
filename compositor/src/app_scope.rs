//! Each app WhiteRose starts runs in its own systemd scope,
//! `app-whiterose-<app id>-<n>.scope` in the user's `app.slice` (the
//! freedesktop convention GNOME and KDE follow), instead of in the login's
//! session scope with the compositor, daemon and shell.
//!
//! Why: when memory runs out, systemd-oomd (and the kernel) pick a cgroup to
//! kill. With every app in the session's scope that's the whole session;
//! with a scope each, it's the one app (docs/plans/memory-protection.md).
//! It also gives each app's memory use in its cgroup, for the watcher.
//!
//! **The kill order.** Each app's `oom_score_adj` is raised before it
//! starts (a user can raise it, not lower it; children inherit it): apps
//! +300, protected apps +100 (the terminal, and the apps in the
//! `memory.protected` setting; their scopes also ask systemd-oomd to avoid
//! them), and an app the memory watcher suspects of leaking +800 (whiterosed
//! raises it). The compositor, daemon and shell stay at 0: last in line.
//!
//! `systemd-run --user --scope` creates the scope around its own process and
//! then execs the app, so the app keeps the process id we started (the
//! scratchpads match windows by it) and our environment. Without a systemd
//! user manager (or with WHITEROSE_APP_SCOPES=0: the UI tests), apps start
//! as before.

use std::{
    path::Path,
    process::Command,
    sync::atomic::{AtomicU32, Ordering},
};

/// `oom_score_adj` for apps, protected apps and suspected leakers.
pub const ADJ_APP: i32 = 300;
pub const ADJ_PROTECTED: i32 = 100;
pub const ADJ_SUSPECT: i32 = 800;

/// Whether `app_id` is protected: in `list` (the `memory.protected`
/// setting), or the terminal's program. Matched without case, and by the
/// last part of a reverse-DNS id ("com.mitchellh.ghostty" is "ghostty").
pub fn is_protected(app_id: &str, list: &[String], terminal: &str) -> bool {
    let short = |s: &str| s.rsplit('.').next().unwrap_or(s).to_ascii_lowercase();
    let id = app_id.to_ascii_lowercase();
    let term = terminal.split_whitespace().next().map(id_for).unwrap_or_default();
    let mut names = list.iter().map(String::as_str).chain(std::iter::once(term.as_str())).filter(|n| !n.is_empty());
    names.any(|n| n.eq_ignore_ascii_case(&id) || short(n) == short(&id))
}

/// Whether apps get scopes here: systemd-run on PATH and the user manager's
/// socket present (checked once).
pub fn available() -> bool {
    static CHECKED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *CHECKED.get_or_init(|| {
        if std::env::var("WHITEROSE_APP_SCOPES").is_ok_and(|v| v == "0") {
            return false;
        }
        let manager = std::env::var_os("XDG_RUNTIME_DIR")
            .map(|d| Path::new(&d).join("systemd/private").exists())
            .unwrap_or(false);
        let run = std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).any(|d| d.join("systemd-run").is_file()))
            .unwrap_or(false);
        manager && run
    })
}

/// A unit name part as systemd-escape makes it: letters, digits, `:`, `_`
/// and `.` (not leading) kept, everything else `\xNN`.
pub fn escape(s: &str) -> String {
    let mut out = String::new();
    for (i, b) in s.bytes().enumerate() {
        match b {
            b'a'..=b'z' | b'A'..=b'Z' | b'0'..=b'9' | b':' | b'_' => out.push(b as char),
            b'.' if i > 0 => out.push('.'),
            _ => out.push_str(&format!("\\x{b:02x}")),
        }
    }
    if out.is_empty() {
        out.push_str("app");
    }
    out
}

/// An app id for a command line with none of its own: the program's name
/// ("/usr/bin/ghostty --x" → "ghostty").
pub fn id_for(program: &str) -> String {
    Path::new(program).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "app".into())
}

/// The scope's unit name: `app-whiterose-<id>-<pid>-<n>.scope` (unique: our
/// pid and a counter, so two of the same app don't clash).
pub fn unit(app_id: &str, n: u32) -> String {
    format!("app-whiterose-{}-{}-{n}.scope", escape(app_id), std::process::id())
}

/// The command that starts `argv` (as app `app_id`) in a scope of its own,
/// or plainly when scopes aren't available; `protected` puts it further
/// back in the kill order.
pub fn command(app_id: &str, argv: &[String], protected: bool) -> Option<Command> {
    let (program, args) = argv.split_first()?;
    let mut c = if available() {
        static N: AtomicU32 = AtomicU32::new(0);
        let mut c = Command::new("systemd-run");
        c.args(scope_args(app_id, N.fetch_add(1, Ordering::Relaxed), protected)).arg(program).args(args);
        c
    } else {
        let mut c = Command::new(program);
        c.args(args);
        c
    };
    raise_oom_score(&mut c, if protected { ADJ_PROTECTED } else { ADJ_APP });
    Some(c)
}

/// Sets the started process's oom_score_adj before it runs anything (so
/// whatever it starts straight away has it too).
fn raise_oom_score(c: &mut Command, adj: i32) {
    use std::os::unix::process::CommandExt;
    // Made before the fork: between fork and exec only async-signal-safe
    // calls (open, write, close), no allocation.
    let text: &'static [u8] = match adj {
        ADJ_PROTECTED => b"100",
        ADJ_SUSPECT => b"800",
        _ => b"300",
    };
    // Safety: only open/write/close on a static path and static bytes.
    unsafe {
        c.pre_exec(move || {
            let fd = libc::open(c"/proc/self/oom_score_adj".as_ptr(), libc::O_WRONLY | libc::O_CLOEXEC);
            if fd >= 0 {
                libc::write(fd, text.as_ptr().cast(), text.len());
                libc::close(fd);
            }
            Ok(())
        });
    }
}

/// systemd-run's options, before the app's argv.
fn scope_args(app_id: &str, n: u32, protected: bool) -> Vec<String> {
    let mut a: Vec<String> = vec![
        "--user".into(),
        "--scope".into(),
        "--quiet".into(),
        // Gone from systemd's list once it's exited, even if it failed.
        "--collect".into(),
        "--slice=app.slice".into(),
        format!("--unit={}", unit(app_id, n)),
    ];
    if protected {
        // systemd-oomd picks another cgroup when it can.
        a.push("--property=ManagedOOMPreference=avoid".into());
    }
    a.push("--".into());
    a
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escapes_like_systemd() {
        assert_eq!(escape("org.mozilla.firefox"), "org.mozilla.firefox");
        assert_eq!(escape("whiterose-files"), "whiterose\\x2dfiles");
        assert_eq!(escape(".hidden"), "\\x2ehidden");
        assert_eq!(escape("a b/c"), "a\\x20b\\x2fc");
        assert_eq!(escape(""), "app");
    }

    #[test]
    fn ids_and_units() {
        assert_eq!(id_for("/usr/bin/ghostty"), "ghostty");
        assert_eq!(id_for("foot"), "foot");
        let u = unit("whiterose-files", 3);
        assert!(u.starts_with("app-whiterose-whiterose\\x2dfiles-") && u.ends_with("-3.scope"), "{u}");
        let a = scope_args("foot", 0, false);
        assert_eq!(&a[..5], ["--user", "--scope", "--quiet", "--collect", "--slice=app.slice"]);
        assert_eq!(a.last().unwrap(), "--", "the app's own options never reach systemd-run");
        assert!(!a.iter().any(|x| x.contains("ManagedOOMPreference")));
        let p = scope_args("foot", 0, true);
        assert!(p.contains(&"--property=ManagedOOMPreference=avoid".to_string()));
        assert_eq!(p.last().unwrap(), "--");
    }

    #[test]
    fn no_command_for_nothing() {
        assert!(command("x", &[], false).is_none());
    }

    #[test]
    fn protected_apps() {
        let list = vec!["org.mozilla.firefox".to_string()];
        assert!(is_protected("com.mitchellh.ghostty", &[], "ghostty -e x"), "the terminal, by its short name");
        assert!(is_protected("ptyxis", &[], "ptyxis --standalone"));
        assert!(is_protected("org.mozilla.Firefox", &list, "foot"));
        assert!(is_protected("firefox", &list, "foot"));
        assert!(!is_protected("org.gnome.Calculator", &list, "foot"));
        assert!(!is_protected("anything", &[], ""));
    }

    #[test]
    fn started_apps_get_their_oom_score() {
        // Plainly (no scope needed to check the score itself).
        let mut c = Command::new("cat");
        c.arg("/proc/self/oom_score_adj");
        raise_oom_score(&mut c, ADJ_PROTECTED);
        let out = c.output().expect("cat runs");
        assert_eq!(String::from_utf8_lossy(&out.stdout).trim(), "100");
        let mut c = Command::new("cat");
        c.arg("/proc/self/oom_score_adj");
        raise_oom_score(&mut c, ADJ_APP);
        assert_eq!(String::from_utf8_lossy(&c.output().unwrap().stdout).trim(), "300");
    }
}
