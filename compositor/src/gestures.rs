//! Touchpad gestures for the desktop (Settings › Mouse & touchpad ›
//! Gestures). A gesture is a number of fingers and what they do: swipe
//! left or right, up, down, pinch in or out. `gestures.bindings` gives
//! each one an action; anything not bound goes to the app under the
//! pointer, as every gesture did before (a browser's pinch to zoom).
//!
//! When some gesture with this many fingers is bound, the compositor
//! holds what the fingers do until it can tell which gesture it is (a
//! little movement, or a little pinch). If it's the app's after all,
//! what was held is handed on, so the app sees the whole gesture.
//!
//! - **Switch workspace** (left or right swipes): the slide follows the
//!   fingers, 1:1 (`anim::Value::drive`), and when they lift it springs
//!   on to the next workspace or back, by how far it went and how fast.
//! - **A shortcut's action**: runs once, when the fingers lift.
//! - **Drag a window** (`gestures.dragWindow`): three fingers on a window
//!   move it, as SUPER+drag does, and the three-finger swipes take four
//!   fingers instead (a three-finger drag and swipe are the same
//!   movement).
//!
//! What decides which gesture it is, and when a swipe switches, is pure
//! and unit-tested here; the rest drives the compositor.

use serde_json::Value;
use smithay::{
    backend::input::ButtonState,
    input::pointer::{
        GestureHoldBeginEvent, GesturePinchBeginEvent, GesturePinchEndEvent, GesturePinchUpdateEvent,
        GestureSwipeBeginEvent, GestureSwipeEndEvent, GestureSwipeUpdateEvent, GrabStartData,
    },
    utils::{Logical, Point, SERIAL_COUNTER},
};

use crate::{keybind::Action, state::WhiteRose};

/// What the fingers do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gesture {
    SwipeHorizontal,
    SwipeUp,
    SwipeDown,
    PinchIn,
    PinchOut,
}

impl Gesture {
    pub const ALL: [Gesture; 5] =
        [Gesture::SwipeHorizontal, Gesture::SwipeUp, Gesture::SwipeDown, Gesture::PinchIn, Gesture::PinchOut];

    /// Its name in `gestures.bindings`, after the fingers ("3-swipe-up").
    pub fn name(self) -> &'static str {
        match self {
            Gesture::SwipeHorizontal => "swipe-horizontal",
            Gesture::SwipeUp => "swipe-up",
            Gesture::SwipeDown => "swipe-down",
            Gesture::PinchIn => "pinch-in",
            Gesture::PinchOut => "pinch-out",
        }
    }

    pub fn is_swipe(self) -> bool {
        matches!(self, Gesture::SwipeHorizontal | Gesture::SwipeUp | Gesture::SwipeDown)
    }
}

/// What a gesture does.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Bound {
    /// Goes to the app under the pointer.
    App,
    /// Left or right: the workspace slide follows the fingers.
    Workspaces,
    /// Up, down or a pinch: the overview follows the fingers open (true)
    /// or closed.
    Overview(bool),
    /// A shortcut's id, run when the fingers lift.
    Shortcut(String),
}

/// `gestures.bindings` and `gestures.dragWindow`.
#[derive(Debug, Clone, PartialEq)]
pub struct GestureSettings {
    bindings: Vec<((u32, Gesture), Bound)>,
    pub drag_window: bool,
}

impl Default for GestureSettings {
    /// Three fingers: left and right switch workspace, up or a pinch in
    /// open the overview, down closes it.
    fn default() -> Self {
        GestureSettings {
            bindings: vec![
                ((3, Gesture::SwipeHorizontal), Bound::Workspaces),
                ((3, Gesture::SwipeUp), Bound::Overview(true)),
                ((3, Gesture::SwipeDown), Bound::Overview(false)),
                ((3, Gesture::PinchIn), Bound::Overview(true)),
            ],
            drag_window: false,
        }
    }
}

impl GestureSettings {
    /// Reads `gestures.bindings` ({"3-swipe-up": "launcher", ...}).
    /// What doesn't parse is left out (the daemon checks it already).
    pub fn set_bindings(&mut self, v: &Value) {
        self.bindings = v
            .as_object()
            .map(|m| {
                m.iter()
                    .filter_map(|(k, a)| {
                        let (n, g) = k.split_once('-')?;
                        let fingers: u32 = n.parse().ok().filter(|n| (2..=4).contains(n))?;
                        let g = Gesture::ALL.into_iter().find(|x| x.name() == g)?;
                        if fingers == 2 && g.is_swipe() {
                            return None;
                        }
                        let bound = match a.as_str()? {
                            "app" => Bound::App,
                            "workspaces" if g == Gesture::SwipeHorizontal => Bound::Workspaces,
                            "workspaces" => return None,
                            "overview-open" if g != Gesture::SwipeHorizontal => Bound::Overview(true),
                            "overview-close" if g != Gesture::SwipeHorizontal => Bound::Overview(false),
                            "overview-open" | "overview-close" => return None,
                            id => Bound::Shortcut(id.to_owned()),
                        };
                        Some(((fingers, g), bound))
                    })
                    .collect()
            })
            .unwrap_or_default();
    }

    /// What `fingers` doing `g` does. With drag on, four-finger swipes
    /// do what the three-finger ones are set to.
    pub fn bound(&self, fingers: u32, g: Gesture) -> Bound {
        let fingers = if g.is_swipe() && self.drag_window && fingers == 4 { 3 } else { fingers };
        self.bindings.iter().find(|(k, _)| *k == (fingers, g)).map(|(_, b)| b.clone()).unwrap_or(Bound::App)
    }

    /// Whether any swipe (or pinch) with `fingers` is the desktop's: if
    /// none is, the gesture goes straight to the app.
    pub fn takes(&self, fingers: u32, swipe: bool) -> bool {
        if swipe && self.drag_window && fingers == 3 {
            return true;
        }
        Gesture::ALL.into_iter().filter(|g| g.is_swipe() == swipe).any(|g| self.bound(fingers, g) != Bound::App)
    }
}

/// How far the fingers move (touchpad units) before it's clear which way
/// a swipe goes.
pub const DECIDE: f64 = 12.0;
/// How far a pinch scales before it's clear it's in or out.
pub const PINCH: f64 = 0.12;
/// How far the fingers move for a whole workspace.
pub const DISTANCE: f64 = 300.0;
/// How far a swipe goes to open or close the overview all the way, and
/// how much a pinch scales; and how far along letting go still opens (or
/// closes) it.
pub const OVERVIEW_DISTANCE: f64 = 250.0;
pub const OVERVIEW_PINCH: f64 = 0.4;
pub const OVERVIEW_ENOUGH: f64 = 0.3;
/// Lifted moving at least this fast (workspaces a second) towards the
/// next one, it goes there even if it's not half way.
pub const FLICK: f64 = 1.5;

/// Which swipe it is once the fingers have moved (`dx`, `dy`) far enough.
pub fn swipe_of(dx: f64, dy: f64) -> Option<Gesture> {
    if dx.hypot(dy) < DECIDE {
        return None;
    }
    Some(if dx.abs() >= dy.abs() {
        Gesture::SwipeHorizontal
    } else if dy < 0.0 {
        Gesture::SwipeUp
    } else {
        Gesture::SwipeDown
    })
}

/// Which pinch it is once it has scaled far enough.
pub fn pinch_of(scale: f64) -> Option<Gesture> {
    if scale <= 1.0 - PINCH {
        Some(Gesture::PinchIn)
    } else if scale >= 1.0 + PINCH {
        Some(Gesture::PinchOut)
    } else {
        None
    }
}

/// How far a swipe of `dx` has gone towards another workspace: positive
/// towards the next one (the fingers going left pull it in from the
/// right, as the slide shows it), negative the one before.
pub fn workspace_progress(dx: f64) -> f64 {
    -dx / DISTANCE
}

/// Whether a swipe let go `along` of the way (0 to 1), moving at `flick`
/// workspaces a second towards the next, switches.
pub fn switches(along: f64, flick: f64) -> bool {
    if flick <= -FLICK {
        return false;
    }
    along >= 0.5 || (flick >= FLICK && along > 0.05)
}

/// A swipe or pinch under way.
#[derive(Debug, Default)]
pub enum Live {
    #[default]
    None,
    /// The app's: passed on as it comes.
    Passing,
    /// Held until it's clear which gesture it is: how far it has gone,
    /// and what came so far (handed on if it's the app's).
    Swipe {
        fingers: u32,
        time: u32,
        dx: f64,
        dy: f64,
        held: Vec<(u32, Point<f64, Logical>)>,
    },
    Pinch {
        fingers: u32,
        time: u32,
        held: Vec<(u32, Point<f64, Logical>, f64, f64)>,
    },
    /// Following the fingers between workspaces.
    Workspace {
        dx: f64,
    },
    /// The overview following the fingers open (or closed): how open it
    /// was when they started, and how far they've gone (a swipe's
    /// movement down, or a pinch's scale).
    Overview {
        open: bool,
        from: f64,
        dy: f64,
    },
    /// Runs when the fingers lift.
    Shortcut(Action),
    /// Moving a window (the button held, as for SUPER+drag).
    Drag,
}

const BTN_LEFT: u32 = 0x110;

impl WhiteRose {
    /// Takes `gestures.bindings` and `gestures.dragWindow`.
    pub fn gestures_setting(&mut self, key: &str, value: &Value) {
        match key {
            "gestures.bindings" => self.gestures.set_bindings(value),
            "gestures.dragWindow" => self.gestures.drag_window = value.as_bool().unwrap_or(false),
            _ => {}
        }
    }

    /// The action a gesture bound to shortcut `id` runs.
    fn gesture_action(&self, id: &str) -> Option<Action> {
        crate::keybind::action_by_id(id, &self.keyboard.terminal, &self.keyboard.custom)
    }

    /// What a gesture that's been told apart does: starts it, or hands
    /// what was held to the app.
    fn gesture_decided(&mut self, fingers: u32, g: Gesture) -> Live {
        match self.gestures.bound(fingers, g) {
            Bound::Workspaces => Live::Workspace { dx: 0.0 },
            Bound::Overview(_) if self.overview_style == "off" => Live::Passing,
            Bound::Overview(open) => Live::Overview { open, from: self.overview_shown(), dy: 0.0 },
            Bound::Shortcut(id) => match self.gesture_action(&id) {
                Some(a) => Live::Shortcut(a),
                None => {
                    tracing::warn!(id, "a gesture's shortcut isn't one that exists");
                    Live::Passing
                }
            },
            Bound::App => Live::Passing,
        }
    }

    /// Fingers came down for a swipe. Also driven by tests.
    pub fn swipe_begin(&mut self, fingers: u32, time: u32) {
        self.activity();
        let pointer = self.seat.get_pointer().expect("seat has a pointer");
        self.gesture = if self.is_locked() || pointer.is_grabbed() || !self.gestures.takes(fingers, true) {
            Live::Passing
        } else if self.gestures.drag_window && fingers == 3 {
            self.gesture_drag_begin()
        } else {
            Live::Swipe { fingers, time, dx: 0.0, dy: 0.0, held: Vec::new() }
        };
        if matches!(self.gesture, Live::Passing) {
            self.pass_swipe_begin(fingers, time);
        }
    }

    pub fn swipe_update(&mut self, time: u32, delta: Point<f64, Logical>) {
        self.activity();
        match &mut self.gesture {
            Live::Passing => self.pass_swipe_update(time, delta),
            Live::Swipe { fingers, time: began, dx, dy, held } => {
                *dx += delta.x;
                *dy += delta.y;
                held.push((time, delta));
                let Some(g) = swipe_of(*dx, *dy) else { return };
                let (fingers, began, dx, dy, held) = (*fingers, *began, *dx, *dy, std::mem::take(held));
                self.gesture = self.gesture_decided(fingers, g);
                match self.gesture {
                    Live::Passing => {
                        self.pass_swipe_begin(fingers, began);
                        for (t, d) in held {
                            self.pass_swipe_update(t, d);
                        }
                    }
                    Live::Workspace { .. } => {
                        self.gesture = Live::Workspace { dx: 0.0 };
                        self.workspace_swipe(dx);
                    }
                    Live::Overview { .. } => self.overview_swipe(dy),
                    _ => {}
                }
            }
            Live::Workspace { .. } => self.workspace_swipe(delta.x),
            Live::Overview { .. } => self.overview_swipe(delta.y),
            Live::Drag => self.on_pointer_relative(delta, time),
            Live::Shortcut(_) | Live::Pinch { .. } | Live::None => {}
        }
    }

    pub fn swipe_end(&mut self, time: u32, cancelled: bool) {
        match std::mem::take(&mut self.gesture) {
            Live::Passing => self.pass_swipe_end(time, cancelled),
            // Lifted before it was clear: it was the app's (a small swipe).
            Live::Swipe { fingers, time: began, held, .. } => {
                self.pass_swipe_begin(fingers, began);
                for (t, d) in held {
                    self.pass_swipe_update(t, d);
                }
                self.pass_swipe_end(time, cancelled);
            }
            Live::Workspace { .. } => {
                let now = std::time::Instant::now();
                let (along, flick) =
                    self.workspace_slide.map(|s| (s.along.get(now), s.along.flick(now))).unwrap_or_default();
                self.end_workspace_drive(!cancelled && switches(along, flick));
            }
            Live::Shortcut(a) if !cancelled => self.run_action(a),
            Live::Overview { open, from, .. } => self.overview_let_go(open, from, cancelled),
            Live::Drag => self.on_pointer_button(BTN_LEFT, ButtonState::Released, time),
            _ => {}
        }
    }

    /// The fingers moved `dx` more between workspaces.
    fn workspace_swipe(&mut self, dx: f64) {
        let Live::Workspace { dx: total } = &mut self.gesture else { return };
        *total += dx;
        let p = workspace_progress(*total);
        let ws = self.workspaces.active();
        let to = if p > 0.0 {
            Some(ws + 1).filter(|n| *n < self.workspaces.count())
        } else if p < 0.0 {
            ws.checked_sub(1)
        } else {
            None
        };
        self.drive_workspace(to, p.abs().min(1.0));
    }

    /// Three fingers came down with drag on: moving the window under them,
    /// as SUPER+drag; on no window, the gesture is the app's.
    fn gesture_drag_begin(&mut self) -> Live {
        let at = self.seat.get_pointer().expect("seat has a pointer").current_location();
        let w = self.window_under(at).map(|(w, _)| w.clone()).filter(|w| !self.fullscreen.contains(w));
        let Some(w) = w else { return Live::Passing };
        self.focus_window(Some(w.clone()));
        self.start_move(w, GrabStartData { focus: None, button: BTN_LEFT, location: at }, SERIAL_COUNTER.next_serial());
        Live::Drag
    }

    /// Fingers came down for a pinch. Also driven by tests.
    pub fn pinch_begin(&mut self, fingers: u32, time: u32) {
        self.activity();
        let pointer = self.seat.get_pointer().expect("seat has a pointer");
        self.gesture = if self.is_locked() || pointer.is_grabbed() || !self.gestures.takes(fingers, false) {
            self.pass_pinch_begin(fingers, time);
            Live::Passing
        } else {
            Live::Pinch { fingers, time, held: Vec::new() }
        };
    }

    pub fn pinch_update(&mut self, time: u32, delta: Point<f64, Logical>, scale: f64, rotation: f64) {
        self.activity();
        match &mut self.gesture {
            Live::Passing => self.pass_pinch_update(time, delta, scale, rotation),
            Live::Pinch { fingers, time: began, held } => {
                held.push((time, delta, scale, rotation));
                let Some(g) = pinch_of(scale) else { return };
                let (fingers, began, held) = (*fingers, *began, std::mem::take(held));
                self.gesture = self.gesture_decided(fingers, g);
                match self.gesture {
                    Live::Passing => {
                        self.pass_pinch_begin(fingers, began);
                        for (t, d, s, r) in held {
                            self.pass_pinch_update(t, d, s, r);
                        }
                    }
                    Live::Overview { .. } => self.overview_pinch(scale),
                    _ => {}
                }
            }
            Live::Overview { .. } => self.overview_pinch(scale),
            _ => {}
        }
    }

    pub fn pinch_end(&mut self, time: u32, cancelled: bool) {
        match std::mem::take(&mut self.gesture) {
            Live::Passing => self.pass_pinch_end(time, cancelled),
            Live::Pinch { fingers, time: began, held } => {
                self.pass_pinch_begin(fingers, began);
                for (t, d, s, r) in held {
                    self.pass_pinch_update(t, d, s, r);
                }
                self.pass_pinch_end(time, cancelled);
            }
            Live::Shortcut(a) if !cancelled => self.run_action(a),
            Live::Overview { open, from, .. } => self.overview_let_go(open, from, cancelled),
            _ => {}
        }
    }

    /// A swipe moved `dy` more with the overview following it: up opens,
    /// down closes.
    fn overview_swipe(&mut self, dy: f64) {
        let Live::Overview { open, from, dy: total } = &mut self.gesture else { return };
        *total += dy;
        let way = if *open { -*total } else { *total } / OVERVIEW_DISTANCE;
        let (open, from, way) = (*open, *from, way.clamp(0.0, 1.0));
        self.overview_follow(open, from, way);
    }

    /// A pinch at `scale` with the overview following it: in opens, out
    /// closes.
    fn overview_pinch(&mut self, scale: f64) {
        let Live::Overview { open, from, .. } = self.gesture else { return };
        let way = if open { 1.0 - scale } else { scale - 1.0 } / OVERVIEW_PINCH;
        self.overview_follow(open, from, way.clamp(0.0, 1.0));
    }

    /// The fingers have gone `way` (0 to 1) of the way to opening (or
    /// closing) the overview from `from`.
    fn overview_follow(&mut self, open: bool, from: f64, way: f64) {
        let v = if open { from + (1.0 - from) * way } else { from * (1.0 - way) };
        self.drive_overview(v);
    }

    /// The fingers lifted: open or closed, by how far they took it and how
    /// fast; cancelled, back to how it was.
    fn overview_let_go(&mut self, open: bool, from: f64, cancelled: bool) {
        let now = std::time::Instant::now();
        let (v, flick) = (self.overview.open.get(now), self.overview.open.flick(now));
        let opened = if cancelled {
            from >= 0.5
        } else if open {
            flick > -FLICK && (v >= OVERVIEW_ENOUGH || flick >= FLICK)
        } else {
            !(flick < FLICK && (v <= 1.0 - OVERVIEW_ENOUGH || flick <= -FLICK))
        };
        self.end_overview_drive(opened);
    }

    /// A hold (fingers resting) always goes to the app.
    pub fn hold_begin(&mut self, fingers: u32, time: u32) {
        self.activity();
        let e = GestureHoldBeginEvent { serial: SERIAL_COUNTER.next_serial(), time, fingers };
        self.with_pointer(|p, s| p.gesture_hold_begin(s, &e));
    }

    // To the app under the pointer.

    fn pass_swipe_begin(&mut self, fingers: u32, time: u32) {
        let e = GestureSwipeBeginEvent { serial: SERIAL_COUNTER.next_serial(), time, fingers };
        self.with_pointer(|p, s| p.gesture_swipe_begin(s, &e));
    }

    fn pass_swipe_update(&mut self, time: u32, delta: Point<f64, Logical>) {
        let e = GestureSwipeUpdateEvent { time, delta };
        self.with_pointer(|p, s| p.gesture_swipe_update(s, &e));
    }

    fn pass_swipe_end(&mut self, time: u32, cancelled: bool) {
        let e = GestureSwipeEndEvent { serial: SERIAL_COUNTER.next_serial(), time, cancelled };
        self.with_pointer(|p, s| p.gesture_swipe_end(s, &e));
    }

    fn pass_pinch_begin(&mut self, fingers: u32, time: u32) {
        let e = GesturePinchBeginEvent { serial: SERIAL_COUNTER.next_serial(), time, fingers };
        self.with_pointer(|p, s| p.gesture_pinch_begin(s, &e));
    }

    fn pass_pinch_update(&mut self, time: u32, delta: Point<f64, Logical>, scale: f64, rotation: f64) {
        let e = GesturePinchUpdateEvent { time, delta, scale, rotation };
        self.with_pointer(|p, s| p.gesture_pinch_update(s, &e));
    }

    fn pass_pinch_end(&mut self, time: u32, cancelled: bool) {
        let e = GesturePinchEndEvent { serial: SERIAL_COUNTER.next_serial(), time, cancelled };
        self.with_pointer(|p, s| p.gesture_pinch_end(s, &e));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn settings(v: Value, drag: bool) -> GestureSettings {
        let mut s = GestureSettings { drag_window: drag, ..Default::default() };
        s.set_bindings(&v);
        s
    }

    #[test]
    fn three_fingers_switch_workspace_and_open_the_overview_to_start_with() {
        let s = GestureSettings::default();
        assert_eq!(s.bound(3, Gesture::SwipeHorizontal), Bound::Workspaces);
        assert_eq!(s.bound(3, Gesture::SwipeUp), Bound::Overview(true));
        assert_eq!(s.bound(3, Gesture::SwipeDown), Bound::Overview(false));
        assert_eq!(s.bound(3, Gesture::PinchIn), Bound::Overview(true));
        assert_eq!(s.bound(3, Gesture::PinchOut), Bound::App);
        assert_eq!(s.bound(4, Gesture::SwipeHorizontal), Bound::App);
        assert!(s.takes(3, true) && s.takes(3, false));
        assert!(!s.takes(4, true), "four fingers go straight to apps");
        assert!(!s.takes(2, false), "so does pinch to zoom");
    }

    #[test]
    fn the_overview_is_only_up_down_or_a_pinch() {
        let s = settings(json!({"3-swipe-horizontal": "overview-open", "4-pinch-out": "overview-close"}), false);
        assert_eq!(s.bound(3, Gesture::SwipeHorizontal), Bound::App);
        assert_eq!(s.bound(4, Gesture::PinchOut), Bound::Overview(false));
    }

    #[test]
    fn bindings_read_and_bad_ones_are_left_out() {
        let s = settings(
            json!({"3-swipe-up": "launcher", "2-pinch-in": "close-window", "4-pinch-out": "app",
                   "2-swipe-up": "lock", "3-swipe-down": "workspaces", "9-swipe-up": "lock", "3-twirl": "lock"}),
            false,
        );
        assert_eq!(s.bound(3, Gesture::SwipeUp), Bound::Shortcut("launcher".into()));
        assert_eq!(s.bound(2, Gesture::PinchIn), Bound::Shortcut("close-window".into()));
        assert_eq!(s.bound(4, Gesture::PinchOut), Bound::App);
        assert_eq!(s.bound(3, Gesture::SwipeDown), Bound::App, "only left or right switch workspace");
        assert_eq!(s.bound(3, Gesture::SwipeHorizontal), Bound::App, "the setting replaces the defaults");
        assert!(s.takes(2, false));
    }

    #[test]
    fn with_drag_on_four_fingers_swipe_as_three_did() {
        let s = settings(json!({"3-swipe-horizontal": "workspaces", "4-swipe-up": "lock"}), true);
        assert!(s.takes(3, true), "three fingers drag");
        assert_eq!(s.bound(4, Gesture::SwipeHorizontal), Bound::Workspaces);
        assert_eq!(s.bound(4, Gesture::SwipeUp), Bound::App, "the four-finger row's own swipes are set aside");
        assert_eq!(s.bound(4, Gesture::PinchIn), Bound::App, "pinches are as they were");
    }

    #[test]
    fn a_swipe_is_told_apart_once_it_has_moved() {
        assert_eq!(swipe_of(5.0, 3.0), None);
        assert_eq!(swipe_of(-15.0, 4.0), Some(Gesture::SwipeHorizontal));
        assert_eq!(swipe_of(3.0, -14.0), Some(Gesture::SwipeUp));
        assert_eq!(swipe_of(2.0, 20.0), Some(Gesture::SwipeDown));
        assert_eq!(pinch_of(0.95), None);
        assert_eq!(pinch_of(0.8), Some(Gesture::PinchIn));
        assert_eq!(pinch_of(1.2), Some(Gesture::PinchOut));
    }

    #[test]
    fn fingers_going_left_bring_in_the_next_workspace() {
        assert_eq!(workspace_progress(-150.0), 0.5);
        assert_eq!(workspace_progress(300.0), -1.0);
    }

    #[test]
    fn half_way_or_a_flick_switches() {
        assert!(switches(0.6, 0.0));
        assert!(!switches(0.4, 0.0));
        assert!(switches(0.2, 2.0), "a flick");
        assert!(!switches(0.02, 3.0), "a twitch isn't");
        assert!(!switches(0.7, -2.0), "flicked back");
    }
}
