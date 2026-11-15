//! Shake to find (Settings › Mouse & touchpad): shaking the pointer makes
//! it big for a moment, so you can see where it is. A shake is quick
//! back-and-forth strokes: four turns of direction, each after a stroke of
//! at least `MIN_STROKE`, within `WINDOW`. It grows over `GROW_MS`, stays
//! while you keep shaking (and `HOLD_MS` after), and shrinks back over
//! `SHRINK_MS`. Frames are drawn only while it's bigger than usual.
//!
//! Only where the compositor draws the pointer (real hardware): nested,
//! the host desktop draws it.

use std::{collections::VecDeque, sync::OnceLock, time::Instant};

/// Milliseconds on a clock that only goes forward (since first asked).
pub fn now_ms() -> u64 {
    static START: OnceLock<Instant> = OnceLock::new();
    START.get_or_init(Instant::now).elapsed().as_millis() as u64
}

/// How big it gets.
pub const MAX: f64 = 3.0;
const MIN_STROKE: f64 = 60.0;
const TURNS: usize = 4;
const WINDOW: u64 = 800;
const GROW_MS: f64 = 120.0;
const SHRINK_MS: f64 = 350.0;
const HOLD_MS: u64 = 500;

/// One axis's current stroke: its direction and how far it's gone.
#[derive(Debug, Default, Clone, Copy)]
struct Stroke {
    sign: i8,
    travel: f64,
}

#[derive(Debug)]
pub struct Shake {
    pub enabled: bool,
    strokes: [Stroke; 2],
    /// When each recent turn happened (ms).
    turns: VecDeque<u64>,
    /// Big until then (ms).
    until: u64,
    /// 0 (usual size) to 1 (MAX), and when it was last worked out.
    level: f64,
    at: u64,
    /// A frame is on its way (the backend's timer while animating).
    pub tick_armed: bool,
}

impl Default for Shake {
    fn default() -> Self {
        Shake {
            enabled: true,
            strokes: [Stroke::default(); 2],
            turns: VecDeque::new(),
            until: 0,
            level: 0.0,
            at: 0,
            tick_armed: false,
        }
    }
}

impl Shake {
    /// The pointer moved by (dx, dy) at `now` (ms). True while shaking.
    pub fn motion(&mut self, dx: f64, dy: f64, now: u64) -> bool {
        if !self.enabled {
            return false;
        }
        for (stroke, d) in self.strokes.iter_mut().zip([dx, dy]) {
            if d.abs() < 0.5 {
                continue;
            }
            let sign = if d > 0.0 { 1 } else { -1 };
            if sign == stroke.sign {
                stroke.travel += d.abs();
            } else {
                if stroke.sign != 0 && stroke.travel >= MIN_STROKE {
                    self.turns.push_back(now);
                }
                *stroke = Stroke { sign, travel: d.abs() };
            }
        }
        while self.turns.front().is_some_and(|t| now.saturating_sub(*t) > WINDOW) {
            self.turns.pop_front();
        }
        let shaking = self.turns.len() >= TURNS;
        if shaking {
            self.advance(now);
            self.until = now + HOLD_MS;
        }
        shaking
    }

    /// A button went down: whatever it was (a drag, a selection) isn't a shake.
    pub fn reset(&mut self) {
        self.strokes = [Stroke::default(); 2];
        self.turns.clear();
    }

    fn advance(&mut self, now: u64) {
        let dt = now.saturating_sub(self.at) as f64;
        self.at = now;
        self.level = if now < self.until {
            (self.level + dt / GROW_MS).min(1.0)
        } else {
            (self.level - dt / SHRINK_MS).max(0.0)
        };
    }

    /// How big the pointer is at `now`: 1 usually, up to MAX.
    pub fn factor(&mut self, now: u64) -> f64 {
        self.advance(now);
        let l = self.level;
        // Eased: quick to start, gentle to settle.
        1.0 + (MAX - 1.0) * l * l * (3.0 - 2.0 * l)
    }

    /// Still changing size or big: keep drawing frames.
    pub fn animating(&self) -> bool {
        self.level > 0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Strokes of `len` px left and right, `n` of them, `ms` apart, in
    /// 10 px steps.
    fn shake(s: &mut Shake, start: u64, n: usize, len: f64, ms: u64) -> (bool, u64) {
        let mut t = start;
        let mut any = false;
        for i in 0..n {
            let dir = if i % 2 == 0 { 1.0 } else { -1.0 };
            let steps = (len / 10.0) as u64;
            for _ in 0..steps {
                t += ms / steps;
                any |= s.motion(10.0 * dir, 0.0, t);
            }
        }
        (any, t)
    }

    #[test]
    fn quick_strokes_are_a_shake() {
        let mut s = Shake::default();
        let (shaking, _) = shake(&mut s, 0, 5, 120.0, 80);
        assert!(shaking);
    }

    #[test]
    fn slow_short_or_few_strokes_are_not() {
        let mut s = Shake::default();
        assert!(!shake(&mut s, 0, 5, 120.0, 400).0, "slow");
        let mut s = Shake::default();
        assert!(!shake(&mut s, 0, 9, 30.0, 40).0, "small jiggles");
        let mut s = Shake::default();
        assert!(!shake(&mut s, 0, 4, 120.0, 80).0, "three turns");
        let mut s = Shake { enabled: false, ..Default::default() };
        assert!(!shake(&mut s, 0, 9, 120.0, 80).0, "off");
    }

    #[test]
    fn a_button_starts_again() {
        let mut s = Shake::default();
        let (_, t) = shake(&mut s, 0, 4, 120.0, 80);
        s.reset();
        assert!(!shake(&mut s, t, 2, 120.0, 80).0);
    }

    #[test]
    fn grows_holds_and_shrinks() {
        let mut s = Shake::default();
        assert_eq!(s.factor(0), 1.0);
        let (_, t) = shake(&mut s, 0, 5, 120.0, 80);
        assert!(s.animating());
        // Fully grown a moment later, and held while it lasts.
        assert_eq!(s.factor(t + GROW_MS as u64), MAX);
        assert_eq!(s.factor(t + HOLD_MS - 1), MAX);
        // Then back to its usual size.
        let mid = s.factor(t + HOLD_MS + SHRINK_MS as u64 / 2);
        assert!(mid > 1.0 && mid < MAX, "{mid}");
        assert_eq!(s.factor(t + HOLD_MS + SHRINK_MS as u64 + 1), 1.0);
        assert!(!s.animating());
    }
}
