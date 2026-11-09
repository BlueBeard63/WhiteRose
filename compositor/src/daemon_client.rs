//! The compositor's connection to whiterosed (this session's settings
//! daemon): it follows the display settings ("displays", and a layout being
//! tried out, "displays.test") and applies changes live. Reconnects if the
//! daemon restarts; works without one (built-in defaults).

use std::{
    io::{ErrorKind, Read, Write},
    os::unix::net::UnixStream,
    time::Duration,
};

use serde_json::Value;
use smithay::reexports::calloop::{
    generic::Generic,
    timer::{TimeoutAction, Timer},
    Interest, Mode, PostAction,
};

use crate::{display_config::Displays, state::WhiteRose};

/// A layout on trial: shown until `until` (unix ms) unless confirmed.
#[derive(Debug, Clone, PartialEq)]
pub struct Trial {
    pub config: Displays,
    pub until: u64,
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// Parses "displays.test" (null or {config, until}).
pub fn parse_trial(v: &Value) -> Option<Trial> {
    let config = serde_json::from_value(v.get("config")?.clone()).ok()?;
    Some(Trial { config, until: v.get("until")?.as_f64()? as u64 })
}

impl WhiteRose {
    /// Connects now, and again whenever the connection is lost.
    pub fn connect_daemon(&mut self) {
        let Ok(stream) = UnixStream::connect(&self.daemon_socket) else {
            self.retry_daemon();
            return;
        };
        if stream.set_nonblocking(true).is_err() {
            self.retry_daemon();
            return;
        }
        let Ok(reader) = stream.try_clone() else { return };
        self.daemon = Some(stream);
        tracing::info!("connected to whiterosed");
        let mut buf = Vec::new();
        let inserted =
            self.loop_handle.insert_source(Generic::new(reader, Interest::READ, Mode::Level), move |_, r, state| {
                let mut chunk = [0u8; 16384];
                loop {
                    // Safety: the stream is owned by this source and outlives the call.
                    match unsafe { r.get_mut() }.read(&mut chunk) {
                        Ok(0) => {
                            tracing::warn!("whiterosed went away; using the last settings");
                            state.daemon = None;
                            state.retry_daemon();
                            return Ok(PostAction::Remove);
                        }
                        Ok(n) => {
                            buf.extend_from_slice(&chunk[..n]);
                            while let Some(i) = buf.iter().position(|&b| b == b'\n') {
                                let line: Vec<u8> = buf.drain(..=i).collect();
                                if let Ok(v) = serde_json::from_slice::<Value>(&line) {
                                    state.daemon_message(&v);
                                }
                            }
                            if buf.len() > 8 << 20 {
                                buf.clear(); // a runaway line: drop it
                            }
                        }
                        Err(e) if e.kind() == ErrorKind::WouldBlock => return Ok(PostAction::Continue),
                        Err(_) => {
                            state.daemon = None;
                            state.retry_daemon();
                            return Ok(PostAction::Remove);
                        }
                    }
                }
            });
        if inserted.is_err() {
            self.daemon = None;
        }
    }

    fn retry_daemon(&mut self) {
        let _ = self.loop_handle.insert_source(Timer::from_duration(Duration::from_secs(2)), |_, _, state| {
            if state.daemon.is_none() {
                state.connect_daemon();
            }
            TimeoutAction::Drop
        });
    }

    /// Sends a request (no reply expected).
    pub fn daemon_request(&mut self, req: Value) {
        if let Some(s) = self.daemon.as_mut() {
            let mut line = req.to_string();
            line.push('\n');
            if s.write_all(line.as_bytes()).is_err() {
                self.daemon = None;
            }
        }
    }

    fn daemon_message(&mut self, v: &Value) {
        let mut changed = false;
        match v.get("event").and_then(Value::as_str) {
            Some("settings") => {
                let vals = &v["values"];
                changed |= self.set_display_setting("displays", &vals["displays"]);
                changed |= self.set_display_setting("displays.test", &vals["displays.test"]);
            }
            Some("setting") => {
                if let Some(key) = v["key"].as_str() {
                    changed |= self.set_display_setting(key, &v["value"]);
                }
            }
            _ => {}
        }
        if changed {
            self.apply_displays();
        }
    }

    fn set_display_setting(&mut self, key: &str, value: &Value) -> bool {
        match key {
            "displays" => match serde_json::from_value::<Displays>(value.clone()) {
                Ok(d) if d != self.displays => {
                    self.displays = d;
                    true
                }
                Ok(_) => false,
                Err(e) => {
                    tracing::warn!(%e, "ignoring unreadable display settings");
                    false
                }
            },
            "displays.test" => {
                let t = parse_trial(value);
                if t != self.display_trial {
                    self.display_trial = t;
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trials_parse_and_null_means_none() {
        let t = parse_trial(
            &serde_json::json!({"config": {"monitor": [{"id": "builtin", "scale": 1.5}]}, "until": 1794240000000u64}),
        )
        .unwrap();
        assert_eq!((t.config.monitor[0].scale, t.until), (1.5, 1794240000000));
        assert_eq!(parse_trial(&Value::Null), None);
        assert_eq!(parse_trial(&serde_json::json!({"until": 5})), None);
    }
}
