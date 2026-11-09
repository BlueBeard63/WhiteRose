//! Keyboard settings and shortcuts followed live from the settings
//! database, and shortcuts pausing while Settings records new keys.

mod common;

use common::*;
use smithay::{backend::input::KeyState, input::keyboard::Keycode};
use whiterose_wm::WhiteRose;

// evdev codes + 8.
const SUPER: u32 = 125 + 8;
const KEY_2: u32 = 3 + 8;
const KEY_3: u32 = 4 + 8;
const SPACE: u32 = 57 + 8;

fn key(state: &mut WhiteRose, code: u32, down: bool) {
    state.on_key(Keycode::new(code), if down { KeyState::Pressed } else { KeyState::Released }, 0);
}

fn chord(state: &mut WhiteRose, a: u32, b: u32) {
    key(state, a, true);
    key(state, b, true);
    key(state, b, false);
    key(state, a, false);
}

fn setting(state: &mut WhiteRose, k: &str, v: serde_json::Value) {
    let (kb, sc) = state.keyboard_setting(k, &v);
    if kb {
        state.apply_keyboard();
    }
    if sc {
        state.apply_shortcuts();
    }
}

#[test]
fn changed_shortcuts_take_effect_at_once() {
    let mut h = Headless::new(true);
    let s = &mut h.state;
    s.apply_shortcuts();
    chord(s, SUPER, KEY_2);
    assert_eq!(s.workspaces.active(), 1, "SUPER+2 is workspace 2 by default");

    // Workspaces move to CTRL+1-9 (with SUPER+3 as a plain key now).
    setting(s, "shortcuts.keys", serde_json::json!({"workspace": ["CTRL+1-9"]}));
    chord(s, SUPER, KEY_3);
    assert_eq!(s.workspaces.active(), 1, "the old keys no longer switch");
    chord(s, 29 + 8, KEY_3);
    assert_eq!(s.workspaces.active(), 2, "CTRL+3 is workspace 3");

    // Back to the default.
    setting(s, "shortcuts.keys", serde_json::json!({}));
    chord(s, SUPER, KEY_2);
    assert_eq!(s.workspaces.active(), 1);
}

#[test]
fn shortcuts_pause_while_settings_records_keys() {
    let mut h = Headless::new(true);
    let s = &mut h.state;
    s.apply_shortcuts();
    s.shortcuts_paused = Some(7);
    chord(s, SUPER, KEY_3);
    assert_eq!(s.workspaces.active(), 0, "SUPER+3 reaches Settings instead");
    assert!(s.swallowed.is_empty(), "nothing swallowed, the app sees every key");
    s.shortcuts_paused = None;
    chord(s, SUPER, KEY_3);
    assert_eq!(s.workspaces.active(), 2);
}

#[test]
fn super_space_cycles_layouts_from_settings() {
    let mut h = Headless::new(true);
    let s = &mut h.state;
    s.apply_shortcuts();
    setting(s, "keyboard.layouts", serde_json::json!([{"layout": "gb"}, {"layout": "es"}]));
    let layout = |s: &mut WhiteRose| {
        let kb = s.seat.get_keyboard().unwrap();
        kb.with_xkb_state(s, |ctx| {
            let xkb = ctx.xkb().lock().unwrap();
            xkb.layout_name(xkb.active_layout()).to_string()
        })
    };
    assert_eq!(layout(s), "English (UK)");
    let names: Vec<_> = s.keyboard.view.iter().map(|l| l.name.as_str()).collect();
    assert_eq!(names, ["English (UK)", "Spanish"], "the shell is told their names");
    chord(s, SUPER, SPACE);
    assert_eq!(layout(s), "Spanish");
    assert_eq!(s.keyboard.active, 1, "and which is active");
    chord(s, SUPER, SPACE);
    assert_eq!(layout(s), "English (UK)");
}
