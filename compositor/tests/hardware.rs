//! The parts of the real-hardware backend that don't need hardware:
//! console switching keys and how the pointer moves between screens.

mod common;

use std::{cell::RefCell, rc::Rc};

use common::*;
use smithay::{
    backend::input::KeyState,
    input::keyboard::Keycode,
    output::{Mode, Output, PhysicalProperties, Subpixel},
    utils::{Logical, Point},
};
use whiterose_wm::WhiteRose;

// evdev codes + 8, as xkb numbers them.
const CTRL: u32 = 29 + 8;
const ALT: u32 = 56 + 8;
const F3: u32 = 61 + 8;

fn key(state: &mut WhiteRose, code: u32, down: bool) {
    state.on_key(Keycode::new(code), if down { KeyState::Pressed } else { KeyState::Released }, 0);
}

fn ctrl_alt_f3(state: &mut WhiteRose) {
    for (code, down) in [(CTRL, true), (ALT, true), (F3, true), (F3, false), (ALT, false), (CTRL, false)] {
        key(state, code, down);
    }
}

fn switches(state: &mut WhiteRose) -> Rc<RefCell<Vec<i32>>> {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let s = seen.clone();
    state.vt_switch = Some(Box::new(move |vt| s.borrow_mut().push(vt)));
    seen
}

#[test]
fn ctrl_alt_f3_switches_to_console_3() {
    let mut h = Headless::new(true);
    let seen = switches(&mut h.state);
    ctrl_alt_f3(&mut h.state);
    assert_eq!(*seen.borrow(), vec![3]);
    assert!(h.state.swallowed.is_empty(), "the F3 release was swallowed too");
}

#[test]
fn nested_ctrl_alt_f3_is_left_alone() {
    // No consoles to switch to in a window: the key isn't taken.
    let mut h = Headless::new(true);
    key(&mut h.state, CTRL, true);
    key(&mut h.state, ALT, true);
    key(&mut h.state, F3, true);
    assert!(h.state.swallowed.is_empty());
}

fn p(x: f64, y: f64) -> Point<f64, Logical> {
    (x, y).into()
}

#[test]
fn pointer_crosses_to_the_next_screen_and_stops_at_edges() {
    let mut h = Headless::new(true);
    // A second, shorter screen to the right.
    let right = Output::new(
        "DP-1".into(),
        PhysicalProperties { size: (0, 0).into(), subpixel: Subpixel::Unknown, make: "T".into(), model: "T".into() },
    );
    let mode = Mode { size: (800, 600).into(), refresh: 60_000 };
    right.change_current_state(Some(mode), None, None, Some((W, 0).into()));
    h.state.space.map_output(&right, (W, 0));

    let s = &h.state;
    let w = W as f64;
    // Across the shared edge.
    assert_eq!(s.clamp_to_outputs(p(w - 10.0, 100.0), p(w + 20.0, 100.0)), p(w + 20.0, 100.0));
    // Past the far right edge: stops there.
    assert_eq!(s.clamp_to_outputs(p(w + 790.0, 100.0), p(w + 900.0, 100.0)), p(w + 799.0, 100.0));
    // Below the shorter screen: slides along its bottom.
    assert_eq!(s.clamp_to_outputs(p(w + 100.0, 590.0), p(w + 120.0, 700.0)), p(w + 120.0, 599.0));
    // Above everything.
    assert_eq!(s.clamp_to_outputs(p(10.0, 5.0), p(12.0, -40.0)), p(12.0, 0.0));
}
