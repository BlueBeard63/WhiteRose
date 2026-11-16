//! The compositor's animations: one kind of value that moves, used by
//! everything that moves (scratchpad slides, tab strips, shake to find,
//! switching workspace, and gestures and the overview after them).
//!
//! A [`Value`] is a number, usually 0 to 1 ("how far along"), that is
//! either still, on its way somewhere over a set time with an easing
//! ([`Tween`]), set directly by the fingers (driven), or springing to
//! where it ends up after the fingers let go ([`Spring`]). Asked for at a
//! time, it says where it is then; nothing moves between frames, so an
//! idle desktop has nothing ticking. While any value moves, the frame
//! that draws it asks for the next one (`WhiteRose::animating`).
//!
//! Times are passed in, so everything here is tested without a clock.

use std::time::{Duration, Instant};

/// How a timed move goes from start to end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Curve {
    /// The same speed throughout.
    Linear,
    /// Quick to start, gentle to settle (cubic).
    EaseOut,
    /// Gentle at both ends (smoothstep).
    EaseInOut,
}

impl Curve {
    /// Where the curve is when `t` (0 to 1) of the time has gone.
    pub fn at(self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Curve::Linear => t,
            Curve::EaseOut => 1.0 - (1.0 - t).powi(3),
            Curve::EaseInOut => t * t * (3.0 - 2.0 * t),
        }
    }
}

/// A move from `from` to `to` over `length`, along `curve`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tween {
    pub from: f64,
    pub to: f64,
    pub start: Instant,
    pub length: Duration,
    pub curve: Curve,
}

impl Tween {
    /// How much of the time has gone at `now`, 0 to 1.
    pub fn progress(&self, now: Instant) -> f64 {
        if self.length.is_zero() {
            return 1.0;
        }
        (now.saturating_duration_since(self.start).as_secs_f64() / self.length.as_secs_f64()).clamp(0.0, 1.0)
    }

    pub fn value(&self, now: Instant) -> f64 {
        self.from + (self.to - self.from) * self.curve.at(self.progress(now))
    }

    pub fn done(&self, now: Instant) -> bool {
        self.progress(now) >= 1.0
    }

    /// When it ends.
    pub fn end(&self) -> Instant {
        self.start + self.length
    }
}

/// How stiff a spring is: most of the way in a quarter of a second, and
/// settled (`SETTLED`) in under half a second, never going past its end.
pub const SPRING: f64 = 20.0;
/// Close enough to its end to call it there (in the value's units: these
/// are fractions, so a thousandth of the way).
const SETTLED: f64 = 0.001;

/// A critically damped spring towards `to`: from where it was let go
/// (`from`, moving at `velocity` per second) it closes in as fast as it
/// can without overshooting. Worked out exactly for any time, so it
/// doesn't depend on how often frames come.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Spring {
    pub from: f64,
    pub to: f64,
    pub velocity: f64,
    pub start: Instant,
    pub stiffness: f64,
}

impl Spring {
    fn t(&self, now: Instant) -> f64 {
        now.saturating_duration_since(self.start).as_secs_f64()
    }

    pub fn value(&self, now: Instant) -> f64 {
        let (w, t) = (self.stiffness, self.t(now));
        let x0 = self.from - self.to;
        self.to + (x0 + (self.velocity + w * x0) * t) * (-w * t).exp()
    }

    /// Its speed at `now`, per second.
    pub fn speed(&self, now: Instant) -> f64 {
        let (w, t) = (self.stiffness, self.t(now));
        let x0 = self.from - self.to;
        let b = self.velocity + w * x0;
        (b - w * (x0 + b * t)) * (-w * t).exp()
    }

    pub fn done(&self, now: Instant) -> bool {
        (self.value(now) - self.to).abs() < SETTLED && self.speed(now).abs() < SETTLED * self.stiffness
    }
}

/// Finger movements older than this when they let go don't count
/// towards how fast it was going (they'd stopped).
const FRESH: Duration = Duration::from_millis(80);

#[derive(Debug, Clone, Copy, PartialEq)]
enum Motion {
    Still,
    Tween(Tween),
    Spring(Spring),
    /// Set by the fingers: the last value, when, and the speed then.
    Driven {
        at: Instant,
        speed: f64,
    },
}

/// A number that moves: see the module docs.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Value {
    /// Where it is when still or driven; where it ends up otherwise.
    target: f64,
    motion: Motion,
}

impl Default for Value {
    fn default() -> Self {
        Value::new(0.0)
    }
}

impl Value {
    pub fn new(v: f64) -> Value {
        Value { target: v, motion: Motion::Still }
    }

    /// Where it is at `now`.
    pub fn get(&self, now: Instant) -> f64 {
        match self.motion {
            Motion::Still | Motion::Driven { .. } => self.target,
            Motion::Tween(t) => t.value(now),
            Motion::Spring(s) => s.value(now),
        }
    }

    /// Where it ends up (or is, when still or driven).
    pub fn target(&self) -> f64 {
        self.target
    }

    /// Still moving at `now` on its own (a driven value isn't: it moves
    /// when the fingers do, and each of those draws a frame).
    pub fn moving(&self, now: Instant) -> bool {
        match self.motion {
            Motion::Still | Motion::Driven { .. } => false,
            Motion::Tween(t) => !t.done(now),
            Motion::Spring(s) => !s.done(now),
        }
    }

    /// Being set by the fingers.
    pub fn driven(&self) -> bool {
        matches!(self.motion, Motion::Driven { .. })
    }

    /// When a timed move ends (None: still, driven or springing).
    pub fn ends(&self) -> Option<Instant> {
        match self.motion {
            Motion::Tween(t) => Some(t.end()),
            _ => None,
        }
    }

    /// Puts it at `v` now, without moving there.
    pub fn jump(&mut self, v: f64) {
        *self = Value::new(v);
    }

    /// Moves it to `to` over `length` along `curve`, from wherever it is
    /// at `now`, so a move turned round half-way goes back from there
    /// without a jump. A zero `length` jumps.
    pub fn animate_to(&mut self, to: f64, length: Duration, curve: Curve, now: Instant) {
        if length.is_zero() {
            return self.jump(to);
        }
        let from = self.get(now);
        self.target = to;
        self.motion = Motion::Tween(Tween { from, to, start: now, length, curve });
    }

    /// The fingers put it at `v` at `now`. How fast it's going is kept
    /// from these, for the spring when they let go.
    pub fn drive(&mut self, v: f64, now: Instant) {
        let speed = match self.motion {
            Motion::Driven { at, speed } => {
                let dt = now.saturating_duration_since(at).as_secs_f64();
                if dt <= 0.0 {
                    speed
                } else {
                    let fresh = (v - self.target) / dt;
                    // Smoothed: one jittery event doesn't fling it.
                    if dt > FRESH.as_secs_f64() {
                        fresh
                    } else {
                        0.5 * speed + 0.5 * fresh
                    }
                }
            }
            _ => 0.0,
        };
        self.target = v;
        self.motion = Motion::Driven { at: now, speed };
    }

    /// How fast the fingers were moving it when they last did (0 if they
    /// had stopped, or it isn't driven).
    pub fn flick(&self, now: Instant) -> f64 {
        match self.motion {
            Motion::Driven { at, speed } if now.saturating_duration_since(at) <= FRESH => speed,
            _ => 0.0,
        }
    }

    /// The fingers let go: it springs from where it is to `to`, keeping
    /// the speed they gave it. (Not driven: it springs from where it is,
    /// at rest.)
    pub fn release(&mut self, to: f64, now: Instant) {
        let from = self.get(now);
        let velocity = match self.motion {
            Motion::Spring(s) => s.speed(now),
            _ => self.flick(now),
        };
        self.target = to;
        self.motion = Motion::Spring(Spring { from, to, velocity, start: now, stiffness: SPRING });
    }

    /// Done moving at `now`: becomes still where it ended. True if it
    /// was moving and has just stopped.
    pub fn settle(&mut self, now: Instant) -> bool {
        let done = match self.motion {
            Motion::Tween(t) => t.done(now),
            Motion::Spring(s) => s.done(now),
            _ => false,
        };
        if done {
            self.motion = Motion::Still;
        }
        done
    }
}

impl crate::state::WhiteRose {
    /// Whether anything drawn is moving: the backend asks for the next
    /// frame after each one until nothing is. (The pointer growing for
    /// shake to find is the DRM backend's own: nested, the host draws it.)
    pub fn animating(&self) -> bool {
        self.overview.open.moving(Instant::now())
            || self.scratchpads.animating()
            || crate::tab_strip::animating(self)
            || self.workspace_sliding(Instant::now()).is_some_and(|s| s.along.moving(Instant::now()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn curves_start_at_0_and_end_at_1() {
        for c in [Curve::Linear, Curve::EaseOut, Curve::EaseInOut] {
            assert_eq!(c.at(0.0), 0.0, "{c:?}");
            assert_eq!(c.at(1.0), 1.0, "{c:?}");
            assert_eq!(c.at(2.0), 1.0, "{c:?} past the end");
        }
        assert_eq!(Curve::EaseOut.at(0.5), 0.875, "most of the way by half way");
        assert_eq!(Curve::EaseInOut.at(0.5), 0.5);
    }

    #[test]
    fn a_timed_move_eases_and_ends() {
        let t0 = Instant::now();
        let mut v = Value::new(0.0);
        v.animate_to(1.0, ms(200), Curve::EaseOut, t0);
        assert_eq!(v.get(t0), 0.0);
        assert_eq!(v.get(t0 + ms(100)), 0.875);
        assert!(v.moving(t0 + ms(199)));
        assert!(!v.moving(t0 + ms(200)));
        assert_eq!(v.get(t0 + ms(500)), 1.0);
        assert_eq!(v.ends(), Some(t0 + ms(200)));
        assert!(v.settle(t0 + ms(200)));
        assert_eq!((v.get(t0), v.moving(t0), v.ends()), (1.0, false, None), "still where it ended");
    }

    #[test]
    fn turned_round_it_goes_back_from_where_it_is() {
        let t0 = Instant::now();
        let mut v = Value::new(0.0);
        v.animate_to(1.0, ms(100), Curve::Linear, t0);
        v.animate_to(0.0, ms(100), Curve::Linear, t0 + ms(40));
        assert!((v.get(t0 + ms(40)) - 0.4).abs() < 1e-9, "no jump");
        assert!((v.get(t0 + ms(90)) - 0.2).abs() < 1e-9);
        assert_eq!(v.target(), 0.0);
    }

    #[test]
    fn no_time_is_a_jump() {
        let t0 = Instant::now();
        let mut v = Value::new(0.0);
        v.animate_to(1.0, Duration::ZERO, Curve::EaseOut, t0);
        assert_eq!(v.get(t0), 1.0);
        assert!(!v.moving(t0));
    }

    #[test]
    fn the_fingers_set_it_and_a_flick_carries_on() {
        let t0 = Instant::now();
        let mut v = Value::new(0.0);
        // 0.1 every 10 ms: 10 a second (smoothed, so a little under).
        for i in 0..=5 {
            v.drive(0.1 * i as f64, t0 + ms(10 * i));
        }
        assert!(v.driven() && !v.moving(t0));
        assert!((v.get(t0) - 0.5).abs() < 1e-9, "exactly where the fingers put it");
        let flick = v.flick(t0 + ms(50));
        assert!((9.0..=10.0).contains(&flick), "{flick}");
        // Let go: it springs to 1, leaving at the fingers' speed.
        let let_go = t0 + ms(50);
        v.release(1.0, let_go);
        assert!(!v.driven() && v.moving(let_go));
        assert!((v.get(let_go) - 0.5).abs() < 1e-9, "no jump when let go");
        // From rest it would be at 0.51 after 10 ms; flicked, it's further.
        assert!(v.get(let_go + ms(10)) > 0.57, "carried on by the flick");
    }

    #[test]
    fn fingers_that_stopped_dont_flick() {
        let t0 = Instant::now();
        let mut v = Value::new(0.0);
        v.drive(0.0, t0);
        v.drive(0.5, t0 + ms(10));
        assert_eq!(v.flick(t0 + ms(200)), 0.0, "held still before letting go");
        v.release(0.0, t0 + ms(200));
        assert!(v.get(t0 + ms(210)) < 0.5, "springs back from rest");
    }

    #[test]
    fn a_spring_settles_without_going_past() {
        let t0 = Instant::now();
        let mut v = Value::new(0.3);
        v.release(1.0, t0);
        let mut last = 0.3;
        for i in 1..=40 {
            let x = v.get(t0 + ms(10 * i));
            assert!(x >= last - 1e-12 && x <= 1.0 + 1e-12, "{i}: {x} after {last}");
            last = x;
        }
        assert!(v.get(t0 + ms(250)) > 0.95, "most of the way in a quarter of a second");
        assert!(!v.moving(t0 + ms(500)), "settled within half a second");
        assert!(v.settle(t0 + ms(500)));
        assert_eq!(v.get(t0 + ms(500)), 1.0);
    }

    #[test]
    fn a_hard_flick_the_other_way_still_ends_where_it_should() {
        let t0 = Instant::now();
        let mut v = Value::new(0.5);
        v.drive(0.5, t0);
        v.drive(0.4, t0 + ms(10)); // going back, fast
        v.release(1.0, t0 + ms(10));
        assert!(v.get(t0 + ms(20)) < 0.4, "carries on back at first");
        assert!((v.get(t0 + ms(1000)) - 1.0).abs() < SETTLED);
    }

    #[test]
    fn a_spring_turned_round_keeps_its_speed() {
        let t0 = Instant::now();
        let mut v = Value::new(0.0);
        v.release(1.0, t0);
        let at = t0 + ms(50);
        let x = v.get(at);
        v.release(0.0, at);
        assert!((v.get(at) - x).abs() < 1e-12, "no jump");
        assert!(v.get(at + ms(5)) > x, "still heading up for a moment");
    }
}
