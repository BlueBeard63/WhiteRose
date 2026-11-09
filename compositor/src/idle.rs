//! Idle: lock the screen, and turn the screens off, after a while with no
//! input ("privacy.lockAfter", "privacy.screenOffAfter", in minutes).
//!
//! Input only stamps the time (nothing per key press); one timer wakes when
//! the next deadline is due and re-arms for the one after. A window that
//! asks to keep the screen on (zwp_idle_inhibit_v1: a playing video, a
//! call) holds both off while it's on screen.

use std::time::{Duration, Instant};

use serde_json::Value;
use smithay::{
    reexports::{
        calloop::timer::{TimeoutAction, Timer},
        wayland_server::{protocol::wl_surface::WlSurface, Resource},
    },
    wayland::idle_inhibit::IdleInhibitHandler,
};

use crate::{keybind::Action, state::WhiteRose};

#[derive(Debug)]
pub struct Idle {
    pub last_input: Instant,
    pub lock_after: Option<Duration>,
    pub off_after: Option<Duration>,
    /// The screens are off for being idle (back on at the next input).
    pub screens_off: bool,
    /// Already locked for this idle stretch (don't ask again).
    pub locked: bool,
    /// Surfaces keeping the screen on.
    pub inhibitors: Vec<WlSurface>,
    /// Off nested in another desktop: that desktop handles idle, and input
    /// to other windows there would look like idling here.
    pub enabled: bool,
    timer_armed: bool,
}

impl Default for Idle {
    fn default() -> Self {
        Idle {
            last_input: Instant::now(),
            lock_after: Some(Duration::from_secs(5 * 60)),
            off_after: Some(Duration::from_secs(10 * 60)),
            screens_off: false,
            locked: false,
            inhibitors: Vec::new(),
            enabled: true,
            timer_armed: false,
        }
    }
}

/// What's due after `idle` time without input: (lock, screens off), and how
/// long until the next thing is due (None: nothing more to do).
pub fn due(
    idle: Duration,
    lock_after: Option<Duration>,
    off_after: Option<Duration>,
    locked: bool,
    off: bool,
) -> (bool, bool, Option<Duration>) {
    let lock = !locked && lock_after.is_some_and(|d| idle >= d);
    let screen_off = !off && off_after.is_some_and(|d| idle >= d);
    let next = [(lock_after, locked || lock), (off_after, off || screen_off)]
        .into_iter()
        .filter_map(|(d, done)| d.filter(|_| !done).map(|d| d.saturating_sub(idle)))
        .min();
    (lock, screen_off, next)
}

impl WhiteRose {
    /// Any input: awake again.
    pub fn activity(&mut self) {
        self.idle.last_input = Instant::now();
        self.idle.locked = false;
        if self.idle.screens_off {
            self.idle.screens_off = false;
            tracing::info!("input: screens on");
            self.schedule_redraw();
        }
        if !self.idle.timer_armed {
            self.arm_idle();
        }
    }

    /// Takes "privacy.lockAfter" / "privacy.screenOffAfter"; true if changed.
    pub fn idle_setting(&mut self, key: &str, value: &Value) -> bool {
        let minutes = || value.as_u64().map(|m| (m > 0).then(|| Duration::from_secs(m * 60)));
        let slot = match key {
            "privacy.lockAfter" => &mut self.idle.lock_after,
            "privacy.screenOffAfter" => &mut self.idle.off_after,
            _ => return false,
        };
        match minutes() {
            Some(d) if d != *slot => {
                *slot = d;
                true
            }
            _ => false,
        }
    }

    /// Is a window that's on screen keeping it awake?
    fn idle_inhibited(&mut self) -> bool {
        self.idle.inhibitors.retain(|s| s.is_alive());
        self.idle.inhibitors.iter().any(|s| {
            let window = self.window_for_surface(s);
            window.is_some_and(|w| self.space.element_location(&w).is_some())
        })
    }

    /// (Re)starts the idle timer for the next deadline.
    pub fn arm_idle(&mut self) {
        if !self.idle.enabled {
            return;
        }
        let (_, _, next) = due(
            self.idle.last_input.elapsed(),
            self.idle.lock_after,
            self.idle.off_after,
            self.idle.locked,
            self.idle.screens_off,
        );
        let Some(next) = next else { return };
        self.idle.timer_armed = true;
        let inserted = self.loop_handle.insert_source(Timer::from_duration(next), |_, _, state| {
            state.idle.timer_armed = false;
            state.idle_check();
            TimeoutAction::Drop
        });
        if inserted.is_err() {
            self.idle.timer_armed = false;
        }
    }

    fn idle_check(&mut self) {
        // Kept awake: count from now, and look again in a minute.
        if self.idle_inhibited() {
            self.idle.last_input = Instant::now();
            self.idle.timer_armed = true;
            let _ = self.loop_handle.insert_source(Timer::from_duration(Duration::from_secs(60)), |_, _, state| {
                state.idle.timer_armed = false;
                state.idle_check();
                TimeoutAction::Drop
            });
            return;
        }
        let (lock, off, _) = due(
            self.idle.last_input.elapsed(),
            self.idle.lock_after,
            self.idle.off_after,
            self.idle.locked,
            self.idle.screens_off,
        );
        if lock {
            self.idle.locked = true;
            if !self.is_locked() {
                tracing::info!("idle: locking");
                self.run_action(Action::Lock);
            }
        }
        if off {
            tracing::info!("idle: screens off");
            self.idle.screens_off = true;
            self.schedule_redraw();
        }
        self.arm_idle();
    }
}

impl IdleInhibitHandler for WhiteRose {
    fn inhibit(&mut self, surface: WlSurface) {
        if !self.idle.inhibitors.contains(&surface) {
            self.idle.inhibitors.push(surface);
        }
    }
    fn uninhibit(&mut self, surface: WlSurface) {
        self.idle.inhibitors.retain(|s| s != &surface);
    }
}
smithay::delegate_idle_inhibit!(WhiteRose);

#[cfg(test)]
mod tests {
    use super::*;

    const M: Duration = Duration::from_secs(60);

    #[test]
    fn deadlines_in_order() {
        // 5 min lock, 10 min off.
        assert_eq!(due(2 * M, Some(5 * M), Some(10 * M), false, false), (false, false, Some(3 * M)));
        assert_eq!(due(5 * M, Some(5 * M), Some(10 * M), false, false), (true, false, Some(5 * M)));
        assert_eq!(due(6 * M, Some(5 * M), Some(10 * M), true, false), (false, false, Some(4 * M)), "locked already");
        assert_eq!(due(10 * M, Some(5 * M), Some(10 * M), true, false), (false, true, None));
        assert_eq!(due(11 * M, Some(5 * M), Some(10 * M), true, true), (false, false, None), "all done");
    }

    #[test]
    fn never_means_never() {
        assert_eq!(due(99 * M, None, None, false, false), (false, false, None));
        assert_eq!(due(3 * M, None, Some(2 * M), false, false), (false, true, None));
    }
}
