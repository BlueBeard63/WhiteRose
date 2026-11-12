//! The tiling keys, pressed through the same key path the backends use
//! (keycodes as evdev + 8, the default shortcut table).

mod common;

use common::{tiles::*, *};
use smithay::{backend::input::KeyState, input::keyboard::Keycode};
use whiterose_wm::{layout::Layout, WhiteRose};

const SHIFT: u32 = 42 + 8;
const CTRL: u32 = 29 + 8;
const ESC: u32 = 1 + 8;
const T: u32 = 20 + 8;
const R: u32 = 19 + 8;
const J: u32 = 36 + 8;
const M: u32 = 50 + 8;
const G: u32 = 34 + 8;
const SPACE: u32 = 57 + 8;
const COMMA: u32 = 51 + 8;
const PERIOD: u32 = 52 + 8;
const KEY_LEFT: u32 = 105 + 8;
const KEY_RIGHT: u32 = 106 + 8;
const KEY_DOWN: u32 = 108 + 8;

/// Holds `keys` down in order, then lets go in reverse.
fn press(s: &mut WhiteRose, keys: &[u32]) {
    for k in keys {
        s.on_key(Keycode::new(*k), KeyState::Pressed, 0);
    }
    for k in keys.iter().rev() {
        s.on_key(Keycode::new(*k), KeyState::Released, 0);
    }
}

#[test]
fn super_t_cycles_the_workspace_layout_and_saves_it() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        open(&mut c);
        open(&mut c);
        c.queue.roundtrip(&mut c.app).unwrap();
        ask.run(|s| {
            press(s, &[SUPER, T]);
            assert_eq!(rects(s), vec![Some(LEFT), Some(RIGHT)]);
            assert_eq!(s.tiling.settings.workspaces.get(&0), Some(&Layout::Dwindle), "saved for the workspace");
            // No whiterosed in this test: the save waits for it, the latest
            // value per setting.
            press(s, &[SUPER, T]);
            let waiting =
                serde_json::json!({"op": "set", "key": "windows.layout.workspaces", "value": {"1": "master"}});
            assert_eq!(s.daemon_waiting, vec![waiting]);
            // Master: 55% of the 1416px left after the gap.
            assert_eq!(rects(s), vec![Some((8, 34, 779, 766)), Some((795, 34, 637, 766))]);
            press(s, &[SUPER, T]);
            assert_eq!(rects(s), vec![Some((48, 74, 0, 0)), Some((80, 106, 0, 0))], "floating again");
            // Only this workspace.
            s.switch_workspace(1);
            assert_eq!(s.layout_of(1, &s.space.outputs().next().unwrap().clone()), Layout::Floating);
        });
    });
}

#[test]
fn super_shift_space_floats_a_tile_and_tiles_it_again() {
    let mut h = Headless::new(true);
    two_tiles(&mut h, |s| {
        // The newest window has focus.
        press(s, &[SUPER, SHIFT, SPACE]);
        let ws = all(s);
        assert!(!s.is_tiled(&ws[1]));
        assert_eq!(rects(s), vec![Some(AREA), Some(RIGHT)], "it floats where it was");
        press(s, &[SUPER, SHIFT, SPACE]);
        assert_eq!(rects(s), vec![Some(LEFT), Some(RIGHT)]);
    });
}

#[test]
fn super_arrows_focus_and_super_shift_arrows_move() {
    let mut h = Headless::new(true);
    two_tiles(&mut h, |s| {
        let ws = all(s);
        press(s, &[SUPER, KEY_LEFT]);
        assert_eq!(s.focused_window(), Some(ws[0].clone()));
        press(s, &[SUPER, KEY_LEFT]);
        assert_eq!(s.focused_window(), Some(ws[0].clone()), "nothing further left");
        press(s, &[SUPER, SHIFT, KEY_RIGHT]);
        assert_eq!(rects(s), vec![Some(RIGHT), Some(LEFT)]);
        assert_eq!(s.focused_window(), Some(ws[0].clone()), "focus moves with it");
    });
}

#[test]
fn super_r_resizes_with_the_arrows_until_escape() {
    let mut h = Headless::new(true);
    two_tiles(&mut h, |s| {
        let ws = all(s);
        s.focus_window(Some(ws[0].clone()));
        press(s, &[SUPER, R]);
        assert!(s.tiling.key_resize);
        press(s, &[KEY_RIGHT]);
        press(s, &[KEY_RIGHT]);
        assert_eq!(rects(s), vec![Some((8, 34, 788, 766)), Some((804, 34, 628, 766))]);
        press(s, &[KEY_LEFT]);
        assert_eq!(rects(s)[0], Some((8, 34, 748, 766)));
        press(s, &[ESC]);
        assert!(!s.tiling.key_resize);
        // The arrows are the app's again.
        press(s, &[KEY_RIGHT]);
        assert_eq!(rects(s)[0], Some((8, 34, 748, 766)));
        // Any other key ends it too (and goes to the app as usual).
        press(s, &[SUPER, R]);
        press(s, &[T]);
        assert!(!s.tiling.key_resize);
        assert_eq!(rects(s)[0], Some((8, 34, 748, 766)));
    });
}

#[test]
fn split_and_master_keys() {
    let mut h = Headless::new(true);
    two_tiles(&mut h, |s| {
        press(s, &[SUPER, J]);
        assert_eq!(rects(s), vec![Some((8, 34, 1424, 379)), Some((8, 421, 1424, 379))]);
        set(s, "windows.layout", serde_json::json!("master"));
        let ws = all(s);
        press(s, &[SUPER, M]);
        assert_eq!(rect(s, &ws[1]), Some((8, 34, 779, 766)), "the focused one is the master");
        press(s, &[SUPER, J]);
        assert_eq!(rect(s, &ws[1]), Some((8, 34, 779, 766)), "no split to flip in master");
    });
}

#[test]
fn group_and_tab_keys() {
    let mut h = Headless::new(true);
    two_tiles(&mut h, |s| {
        let ws = all(s);
        // The focused window joins the last one used.
        press(s, &[SUPER, G]);
        assert_eq!(rects(s), vec![None, Some((8, 62, 1424, 738))]);
        press(s, &[SUPER, COMMA]);
        assert_eq!(rects(s), vec![Some((8, 62, 1424, 738)), None]);
        assert_eq!(s.focused_window(), Some(ws[0].clone()));
        press(s, &[SUPER, PERIOD]);
        assert_eq!(s.focused_window(), Some(ws[1].clone()));
        // And leaves again.
        press(s, &[SUPER, G]);
        assert_eq!(rects(s).iter().filter(|r| r.is_some()).count(), 2);
        assert!(s.tiling.strips.is_empty());
    });
}

#[test]
fn floating_windows_move_and_resize_by_keys() {
    let mut h = Headless::new(true);
    // With snapping on, SUPER+SHIFT+arrows snap instead (tests/snap.rs).
    set(&mut h.state, "windows.snap", serde_json::json!(false));
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let (a, _t) = open(&mut c);
        c.attach_buffer(&a, 400, 300);
        ask.run(|s| {
            press(s, &[SUPER, SHIFT, KEY_RIGHT]);
            assert_eq!(rects(s), vec![Some((88, 74, 0, 0))]);
            press(s, &[SUPER, R]);
            press(s, &[KEY_DOWN]);
            press(s, &[ESC]);
            assert_eq!(rects(s), vec![Some((88, 74, 400, 340))]);
            // CTRL is just a modifier in resize mode: it doesn't end it.
            press(s, &[SUPER, R]);
            press(s, &[CTRL]);
            assert!(s.tiling.key_resize);
        });
    });
}
