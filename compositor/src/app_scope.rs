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
/// or plainly when scopes aren't available.
pub fn command(app_id: &str, argv: &[String]) -> Option<Command> {
    let (program, args) = argv.split_first()?;
    if !available() {
        let mut c = Command::new(program);
        c.args(args);
        return Some(c);
    }
    static N: AtomicU32 = AtomicU32::new(0);
    let mut c = Command::new("systemd-run");
    c.args(scope_args(app_id, N.fetch_add(1, Ordering::Relaxed))).arg(program).args(args);
    Some(c)
}

/// systemd-run's options, before the app's argv.
fn scope_args(app_id: &str, n: u32) -> Vec<String> {
    vec![
        "--user".into(),
        "--scope".into(),
        "--quiet".into(),
        // Gone from systemd's list once it's exited, even if it failed.
        "--collect".into(),
        "--slice=app.slice".into(),
        format!("--unit={}", unit(app_id, n)),
        "--".into(),
    ]
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
        let a = scope_args("foot", 0);
        assert_eq!(&a[..5], ["--user", "--scope", "--quiet", "--collect", "--slice=app.slice"]);
        assert_eq!(a.last().unwrap(), "--", "the app's own options never reach systemd-run");
    }

    #[test]
    fn no_command_for_nothing() {
        assert!(command("x", &[]).is_none());
    }
}
