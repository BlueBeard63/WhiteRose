//! Switching workspace slides: the workspace you leave goes out one side
//! of each monitor while the one you go to comes in from the other (to
//! the left for a later workspace, as if they sat in a row). Only the
//! drawing moves: the windows are already where they'll be, so the
//! pointer and keyboard work on the new workspace at once, as with the
//! scratchpad slide. `animations.workspaceSwitch` off: no slide.
//!
//! The slide is an `anim::Value` from 0 (the workspace left, `from`, in
//! place) to 1 (the one gone to, `to`, in place), so the fingers can set
//! it too (touchpad gestures).

use std::time::{Duration, Instant};

use serde_json::Value;

use crate::anim;

/// How long a switch slides for.
pub const SLIDE: Duration = Duration::from_millis(250);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Slide {
    pub from: usize,
    pub to: usize,
    pub along: anim::Value,
}

impl Slide {
    /// A switch from `from` to `to` starting at `now`.
    pub fn new(from: usize, to: usize, now: Instant, length: Duration) -> Slide {
        let mut along = anim::Value::new(0.0);
        along.animate_to(1.0, length, anim::Curve::EaseOut, now);
        Slide { from, to, along }
    }

    /// The switch went back to `from` while sliding: it slides back from
    /// where it is, for its share of `length`.
    pub fn back(&mut self, now: Instant, length: Duration) {
        let way = self.along.get(now);
        self.along.animate_to(0.0, length.mul_f64(way), anim::Curve::EaseOut, now);
    }

    /// Where `ws`'s windows are drawn, as a share of the monitor's width
    /// from where they are (None: `ws` isn't sliding).
    pub fn shift(&self, ws: usize, now: Instant) -> Option<f64> {
        let dir = if self.to > self.from { 1.0 } else { -1.0 };
        let p = self.along.get(now);
        if ws == self.from {
            Some(-p * dir)
        } else if ws == self.to {
            Some((1.0 - p) * dir)
        } else {
            None
        }
    }

    /// Sliding (or held by the fingers) at `now`.
    pub fn going(&self, now: Instant) -> bool {
        self.along.moving(now) || self.along.driven()
    }
}

impl crate::state::WhiteRose {
    /// Takes `animations.workspaceSwitch`.
    pub fn animation_setting(&mut self, key: &str, value: &Value) {
        if key == "animations.workspaceSwitch" {
            self.workspace_slide_length = if value.as_bool().unwrap_or(true) { SLIDE } else { Duration::ZERO };
            if self.workspace_slide_length.is_zero() {
                self.workspace_slide = None;
            }
        }
    }

    /// Slides from the workspace that was active to `to` (just switched
    /// to). Going back to where a slide came from turns it round.
    pub(crate) fn slide_workspace(&mut self, from: usize, to: usize) {
        let (now, length) = (Instant::now(), self.workspace_slide_length);
        // The fingers took it there: it springs on from where they let go.
        if let Some(s) = self.workspace_slide.as_mut().filter(|s| s.along.driven() && (s.from, s.to) == (from, to)) {
            s.along.release(1.0, now);
            self.schedule_redraw();
            return;
        }
        if length.is_zero() {
            return;
        }
        match self.workspace_slide.as_mut().filter(|s| s.going(now)) {
            Some(s) if s.from == to && s.to == from => s.back(now, length),
            _ => self.workspace_slide = Some(Slide::new(from, to, now, length)),
        }
        self.schedule_redraw();
    }

    /// The fingers have taken the slide `along` (0 to 1) of the way to
    /// workspace `to` (None: there's no workspace that way). A slide
    /// between the same two still going (springing back) carries on from
    /// where it is.
    pub(crate) fn drive_workspace(&mut self, to: Option<usize>, along: f64) {
        let (now, from) = (Instant::now(), self.workspaces.active());
        match to {
            None => {
                if self.workspace_slide.is_some_and(|s| s.from == from) {
                    self.workspace_slide = None;
                }
            }
            Some(to) => {
                let same = self.workspace_slide.is_some_and(|s| (s.from, s.to) == (from, to));
                if !same {
                    self.workspace_slide = Some(Slide { from, to, along: anim::Value::new(0.0) });
                }
                if let Some(s) = self.workspace_slide.as_mut() {
                    s.along.drive(along.clamp(0.0, 1.0), now);
                }
            }
        }
        self.schedule_redraw();
    }

    /// The fingers lifted: on to the workspace they were taking it to
    /// (`switch`), or back.
    pub(crate) fn end_workspace_drive(&mut self, switch: bool) {
        let Some(s) = self.workspace_slide.filter(|s| s.along.driven()) else { return };
        if switch {
            self.switch_workspace(s.to);
        } else if let Some(s) = self.workspace_slide.as_mut() {
            s.along.release(0.0, Instant::now());
        }
        self.schedule_redraw();
    }

    /// The slide going on now, if one is.
    pub fn workspace_sliding(&self, now: Instant) -> Option<&Slide> {
        self.workspace_slide.as_ref().filter(|s| s.going(now))
    }

    /// How far across (logical px) to draw a window of workspace `ws` on a
    /// monitor `width` wide at `now`: 0 when nothing slides.
    pub fn workspace_shift(&self, ws: Option<usize>, width: i32, now: Instant) -> i32 {
        let shift = ws.and_then(|ws| self.workspace_sliding(now)?.shift(ws, now));
        shift.map_or(0, |s| (s * width as f64).round() as i32)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn a_later_workspace_comes_in_from_the_right() {
        let t0 = Instant::now();
        let s = Slide::new(1, 3, t0, SLIDE);
        assert_eq!(s.shift(1, t0), Some(-0.0));
        assert_eq!(s.shift(3, t0), Some(1.0), "just off the right edge");
        assert_eq!(s.shift(2, t0), None, "workspaces in between aren't drawn");
        let mid = s.shift(3, t0 + ms(125)).unwrap();
        assert!(mid > 0.0 && mid < 0.5, "eased: more than half way in by half time ({mid})");
        assert_eq!(s.shift(1, t0 + SLIDE), Some(-1.0), "gone off the left");
        assert_eq!(s.shift(3, t0 + SLIDE), Some(0.0));
        assert!(s.going(t0 + ms(249)) && !s.going(t0 + SLIDE));
    }

    #[test]
    fn an_earlier_one_comes_in_from_the_left() {
        let t0 = Instant::now();
        let s = Slide::new(4, 0, t0, SLIDE);
        assert_eq!(s.shift(0, t0), Some(-1.0));
        assert_eq!(s.shift(4, t0 + SLIDE), Some(1.0));
    }

    #[test]
    fn going_back_mid_way_turns_round() {
        let t0 = Instant::now();
        let mut s = Slide::new(0, 1, t0, SLIDE);
        let at = t0 + ms(50);
        let was = s.shift(1, at).unwrap();
        s.back(at, SLIDE);
        assert!((s.shift(1, at).unwrap() - was).abs() < 1e-9, "no jump");
        assert_eq!(s.shift(1, at + SLIDE), Some(1.0), "back off the right");
        assert_eq!(s.shift(0, at + SLIDE), Some(0.0));
    }
}
