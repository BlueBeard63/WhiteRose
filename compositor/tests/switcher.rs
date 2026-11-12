//! ALT+TAB and SUPER+TAB driven by real key events through the seat's
//! keyboard (xkb keycodes = evdev + 8), against windows from a real client.

mod common;

use common::*;
use smithay::{backend::input::KeyState, input::keyboard::Keycode};
use whiterose_wm::{switcher::Mode, WhiteRose};

const ALT: u32 = 56 + 8;
const SUPER: u32 = 125 + 8;
const SHIFT: u32 = 42 + 8;
const TAB: u32 = 15 + 8;
const GRAVE: u32 = 41 + 8;
const ESC: u32 = 1 + 8;

fn key(state: &mut WhiteRose, code: u32, down: bool) {
    state.on_key(Keycode::new(code), if down { KeyState::Pressed } else { KeyState::Released }, 0);
}
fn tap(state: &mut WhiteRose, code: u32) {
    key(state, code, true);
    key(state, code, false);
}
fn focused_title(state: &WhiteRose) -> String {
    state.focused_window().and_then(|w| whiterose_wm::ipc::toplevel_ids(&w).1).unwrap_or_default()
}

/// Opens term-1, browser, term-2 (so MRU is term-2, browser, term-1).
fn three_windows(h: &mut Headless) {
    h.run(
        |conn| {
            let mut c = Client::connect(&conn);
            for (app, title) in [("term", "term-1"), ("browser", "browser"), ("term", "term-2")] {
                c.app.toplevel_configured = false;
                let (_s, t) = c.open_window();
                t.set_app_id(app.into());
                t.set_title(title.into());
                c.queue.roundtrip(&mut c.app).unwrap();
                std::mem::forget(t); // keep the window alive after the client closure
            }
        },
        |state, ()| assert_eq!(focused_title(state), "term-2"),
    );
}

#[test]
fn alt_tab_quick_tap_goes_back_to_the_previous_app() {
    let mut h = Headless::new(true);
    three_windows(&mut h);
    let s = &mut h.state;
    key(s, ALT, true);
    tap(s, TAB);
    let (sw, held) = s.switcher.clone().expect("ALT+TAB opened the app switcher");
    assert_eq!(sw.mode, Mode::Apps);
    assert_eq!(sw.items.len(), 2, "two apps: term (2 windows) and browser");
    assert!(held.alt && !held.shift);
    key(s, ALT, false);
    assert!(s.switcher.is_none(), "releasing ALT closes it");
    assert_eq!(focused_title(s), "browser");
}

#[test]
fn holding_alt_and_tabbing_moves_and_wraps() {
    let mut h = Headless::new(true);
    three_windows(&mut h);
    let s = &mut h.state;
    key(s, ALT, true);
    tap(s, TAB); // opens on browser
    tap(s, TAB); // wraps to term
    assert_eq!(s.switcher.as_ref().unwrap().0.selected, 0);
    key(s, SHIFT, true);
    tap(s, TAB); // shift+tab: back to browser
    key(s, SHIFT, false);
    assert_eq!(s.switcher.as_ref().unwrap().0.selected, 1);
    assert!(s.switcher.is_some(), "releasing SHIFT alone doesn't commit");
    key(s, ALT, false);
    assert_eq!(focused_title(s), "browser");
}

#[test]
fn backtick_picks_another_window_of_the_app() {
    let mut h = Headless::new(true);
    three_windows(&mut h);
    let s = &mut h.state;
    key(s, ALT, true);
    tap(s, TAB); // browser
    tap(s, TAB); // term
    tap(s, GRAVE); // term's second window: term-1
    key(s, ALT, false);
    assert_eq!(focused_title(s), "term-1");
}

#[test]
fn escape_cancels_without_changing_focus() {
    let mut h = Headless::new(true);
    three_windows(&mut h);
    let s = &mut h.state;
    key(s, ALT, true);
    tap(s, TAB);
    tap(s, ESC);
    assert!(s.switcher.is_none());
    key(s, ALT, false);
    assert_eq!(focused_title(s), "term-2", "focus unchanged after Esc");
}

#[test]
fn super_tab_switches_window_by_window() {
    let mut h = Headless::new(true);
    three_windows(&mut h);
    let s = &mut h.state;
    key(s, SUPER, true);
    tap(s, TAB); // browser (previous window)
    tap(s, TAB); // term-1
    let (sw, _) = s.switcher.clone().unwrap();
    assert_eq!(sw.mode, Mode::Windows);
    assert_eq!(sw.items.len(), 3);
    key(s, SUPER, false); // releasing SUPER commits
    assert!(s.switcher.is_none());
    assert_eq!(focused_title(s), "term-1");
    // The SUPER release after switching must not open the launcher.
    assert!(!s.logo_tap_pending);
}

#[test]
fn keys_typed_while_switching_do_not_reach_the_app() {
    let mut h = Headless::new(true);
    three_windows(&mut h);
    let s = &mut h.state;
    key(s, ALT, true);
    tap(s, TAB);
    let swallowed_before = s.swallowed.len();
    key(s, GRAVE, true);
    assert_eq!(s.swallowed.len(), swallowed_before + 1, "grave press intercepted");
    key(s, GRAVE, false);
    key(s, ALT, false);
    assert!(s.swallowed.is_empty(), "every swallowed press had its release swallowed too");
}
