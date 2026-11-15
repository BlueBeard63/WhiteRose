//! Keeps the shell (bar, panels, lock screen) and whiterosed (settings,
//! system state) running: if one exits, it is started again. This matters
//! most for the shell while the screen is locked: the compositor stays locked
//! when the shell dies, and only a restarted shell can show the lock screen
//! again and unlock. A process that keeps crashing is given up on (5 exits
//! within a minute) instead of spinning.

use std::{
    collections::VecDeque,
    process::Command,
    time::{Duration, Instant},
};

use smithay::reexports::calloop::{
    channel::{channel, Event},
    timer::{TimeoutAction, Timer},
};

use crate::state::WhiteRose;

const MAX_RESTARTS: usize = 5;
const WINDOW: Duration = Duration::from_secs(60);

/// When to restart: at most MAX_RESTARTS exits within WINDOW.
#[derive(Default)]
pub struct RestartPolicy {
    exits: VecDeque<Instant>,
}

impl RestartPolicy {
    pub fn should_restart(&mut self, now: Instant) -> bool {
        self.exits.push_back(now);
        while self.exits.front().is_some_and(|t| now.duration_since(*t) > WINDOW) {
            self.exits.pop_front();
        }
        self.exits.len() <= MAX_RESTARTS
    }
}

/// The daemon and shell belong to this session: when the compositor exits,
/// however it exits (even killed), the kernel sends them SIGTERM, so none is
/// left behind holding the session's sockets.
fn die_with_us(cmd: &mut Command) {
    use std::os::unix::process::CommandExt;
    // Safety: prctl is async-signal-safe, and nothing else runs between fork
    // and exec here.
    unsafe {
        cmd.pre_exec(|| {
            if libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }
}

/// How a supervised process ended, when it wasn't a clean exit (code 0).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Exit {
    Code(i32),
    Signal(i32),
}

impl Exit {
    fn of(status: &std::process::ExitStatus) -> Option<Exit> {
        use std::os::unix::process::ExitStatusExt;
        match (status.code(), status.signal()) {
            (Some(0), _) => None,
            (Some(c), _) => Some(Exit::Code(c)),
            // Asked to stop (logging out, a restart): not a crash.
            (None, Some(libc::SIGTERM | libc::SIGHUP | libc::SIGINT)) => None,
            (None, Some(s)) => Some(Exit::Signal(s)),
            (None, None) => None,
        }
    }
}

/// Where crash records go: $XDG_STATE_HOME/whiterose/crashes (the format
/// is daemon/internal/crash's: a folder `<ms>-<what>` with crash.json;
/// whiterosed takes a snapshot into it and keeps the newest 5).
fn crashes_dir() -> Option<std::path::PathBuf> {
    let state = std::env::var_os("XDG_STATE_HOME")
        .filter(|v| !v.is_empty())
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| std::path::PathBuf::from(h).join(".local/state")))?;
    Some(state.join("whiterose/crashes"))
}

fn crash_record(what: &str, exit: Exit, ms: u128, version: &str) -> serde_json::Value {
    let mut r = serde_json::json!({ "what": what, "time": ms as u64 });
    match exit {
        Exit::Code(c) => r["code"] = c.into(),
        Exit::Signal(s) => r["signal"] = s.into(),
    }
    if !version.is_empty() {
        r["version"] = version.into();
    }
    r
}

fn write_crash_record(what: &str, exit: Exit) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
    let Some(dir) = crashes_dir() else { return Ok(()) };
    let ms = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    let dir = dir.join(format!("{ms:013}-{what}"));
    std::fs::DirBuilder::new().recursive(true).mode(0o700).create(&dir)?;
    let version = std::env::var("WHITEROSE_VERSION").unwrap_or_default();
    let body = serde_json::to_vec_pretty(&crash_record(what, exit, ms, &version))?;
    let mut f =
        std::fs::OpenOptions::new().write(true).create(true).truncate(true).mode(0o600).open(dir.join("crash.json"))?;
    std::io::Write::write_all(&mut f, &body)
}

impl WhiteRose {
    /// Starts `command` and restarts it whenever it exits (see module docs).
    /// `name` is for logs and keeps each process's restart count apart.
    pub fn supervise(&mut self, name: &'static str, command: String) {
        let mut parts = command.split_whitespace();
        let Some(program) = parts.next() else { return };
        let mut cmd = Command::new(program);
        cmd.args(parts).env("WAYLAND_DISPLAY", &self.socket_name).envs(self.ipc_env());
        die_with_us(&mut cmd);
        let child = cmd.spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(err) => {
                tracing::error!(name, command, %err, "failed to start");
                return;
            }
        };
        tracing::info!(name, command, "started");
        let (tx, rx) = channel::<(String, Option<Exit>)>();
        std::thread::spawn(move || {
            let (status, exit) = match child.wait() {
                Ok(s) => (s.to_string(), Exit::of(&s)),
                Err(e) => (e.to_string(), None),
            };
            let _ = tx.send((status, exit));
        });
        let inserted = self.loop_handle.insert_source(rx, move |event, _, state| {
            let Event::Msg((status, exit)) = event else { return };
            // Died (not a clean exit): noted for a bug report.
            if let Some(exit) = exit {
                if let Err(err) = write_crash_record(name, exit) {
                    tracing::warn!(name, %err, "can't write the crash record");
                }
            }
            if !state.restarts.entry(name).or_default().should_restart(Instant::now()) {
                tracing::error!(name, status, "keeps exiting; not restarting it again");
                return;
            }
            tracing::warn!(name, status, locked = state.is_locked(), "exited; restarting it");
            let command = command.clone();
            let _ =
                state.loop_handle.insert_source(Timer::from_duration(Duration::from_secs(1)), move |_, _, state| {
                    state.supervise(name, command.clone());
                    TimeoutAction::Drop
                });
        });
        if let Err(err) = inserted {
            tracing::warn!(name, %err, "can't watch the process");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crash_records() {
        use std::os::unix::process::ExitStatusExt;
        assert_eq!(Exit::of(&std::process::ExitStatus::from_raw(0)), None);
        assert_eq!(Exit::of(&std::process::ExitStatus::from_raw(3 << 8)), Some(Exit::Code(3)));
        assert_eq!(Exit::of(&std::process::ExitStatus::from_raw(11)), Some(Exit::Signal(11)));
        assert_eq!(Exit::of(&std::process::ExitStatus::from_raw(15)), None, "asked to stop");
        assert_eq!(Exit::of(&std::process::ExitStatus::from_raw(9)), Some(Exit::Signal(9)), "killed (out of memory)");
        // The same fields as daemon/internal/crash.Record.
        assert_eq!(
            crash_record("shell", Exit::Signal(11), 1794571766000, "0.3.0"),
            serde_json::json!({ "what": "shell", "time": 1794571766000u64, "signal": 11, "version": "0.3.0" })
        );
        assert_eq!(
            crash_record("daemon", Exit::Code(2), 5, ""),
            serde_json::json!({ "what": "daemon", "time": 5, "code": 2 })
        );
    }

    #[test]
    fn restarts_until_it_crashes_too_often() {
        let mut p = RestartPolicy::default();
        let t0 = Instant::now();
        for i in 0..MAX_RESTARTS {
            assert!(p.should_restart(t0 + Duration::from_secs(i as u64)), "exit {i}");
        }
        assert!(!p.should_restart(t0 + Duration::from_secs(10)), "6th exit within a minute: give up");
        // Exits spread out over time are always restarted.
        let mut p = RestartPolicy::default();
        for i in 0..20 {
            assert!(p.should_restart(t0 + Duration::from_secs(i * 30)), "exit {i}");
        }
    }
}
