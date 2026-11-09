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

impl WhiteRose {
    /// Starts `command` and restarts it whenever it exits (see module docs).
    /// `name` is for logs and keeps each process's restart count apart.
    pub fn supervise(&mut self, name: &'static str, command: String) {
        let mut parts = command.split_whitespace();
        let Some(program) = parts.next() else { return };
        let child =
            Command::new(program).args(parts).env("WAYLAND_DISPLAY", &self.socket_name).envs(self.ipc_env()).spawn();
        let mut child = match child {
            Ok(c) => c,
            Err(err) => {
                tracing::error!(name, command, %err, "failed to start");
                return;
            }
        };
        tracing::info!(name, command, "started");
        let (tx, rx) = channel::<String>();
        std::thread::spawn(move || {
            let status = child.wait().map(|s| s.to_string()).unwrap_or_else(|e| e.to_string());
            let _ = tx.send(status);
        });
        let inserted = self.loop_handle.insert_source(rx, move |event, _, state| {
            let Event::Msg(status) = event else { return };
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
