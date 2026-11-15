//! Snapping floating windows: dragged to edges and corners, and stepped
//! with SUPER+SHIFT+arrows, through the same pointer and key paths the
//! backends use.

mod common;

use common::{tiles::*, *};
use serde_json::json;
use smithay::{
    backend::input::KeyState, input::keyboard::Keycode,
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as ServerState,
};
use whiterose_wm::WhiteRose;

const SHIFT: u32 = 42 + 8;
const KEY_UP: u32 = 103 + 8;
const KEY_LEFT: u32 = 105 + 8;
const KEY_RIGHT: u32 = 106 + 8;
const KEY_DOWN: u32 = 108 + 8;

/// The usable area below the bar, whole (what maximize fills).
const USABLE: (i32, i32, i32, i32) = (0, 26, W, H - 26);
/// The top-left and bottom-left quarters, with the 8px gaps.
const TOP_LEFT: (i32, i32, i32, i32) = (8, 34, 708, 379);
const BOTTOM_LEFT: (i32, i32, i32, i32) = (8, 421, 708, 379);
/// Where the window floats on its own: the cascade, at the size it drew.
const OWN: (i32, i32, i32, i32) = (48, 74, 400, 300);

fn press(s: &mut WhiteRose, keys: &[u32]) {
    for k in keys {
        s.on_key(Keycode::new(*k), KeyState::Pressed, 0);
    }
    for k in keys.iter().rev() {
        s.on_key(Keycode::new(*k), KeyState::Released, 0);
    }
}

fn arrow(s: &mut WhiteRose, key: u32) {
    press(s, &[SUPER, SHIFT, key]);
}

fn the_window(s: &WhiteRose) -> smithay::desktop::Window {
    all(s).remove(0)
}

fn where_(s: &WhiteRose) -> Option<(i32, i32, i32, i32)> {
    rect(s, &the_window(s))
}

fn maximized(s: &WhiteRose) -> bool {
    the_window(s).toplevel().unwrap().with_pending_state(|p| p.states.contains(ServerState::Maximized))
}

/// A bar and one floating 400x300 window, then `test`.
fn one_window(h: &mut Headless, test: impl FnOnce(&mut WhiteRose) + Send + 'static) {
    h.drive(move |conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let (a, _t) = open(&mut c);
        c.attach_buffer(&a, 400, 300);
        ask.run(move |s| {
            assert_eq!(where_(s), Some((48, 74, 0, 0)));
            test(s);
        });
    });
}

#[test]
fn dragged_to_a_side_it_fills_that_half() {
    let mut h = Headless::new(true);
    one_window(&mut h, |s| {
        let preview = super_drag(s, BTN_LEFT, (100.0, 100.0), (0.0, 400.0));
        assert_eq!(preview, Some(LEFT), "the preview shows the half");
        assert_eq!(where_(s), Some(LEFT));
        assert!(told_tiled(&the_window(s)), "told it's tiled, so it drops its shadows");
        assert!(s.snap_of(&the_window(s)).is_some());
        assert_eq!(s.tiling.preview.rect, None, "the preview goes when it's dropped");

        let preview = super_drag(s, BTN_LEFT, (100.0, 100.0), (1439.0, 400.0));
        assert_eq!(preview, Some(RIGHT));
        assert_eq!(where_(s), Some(RIGHT));
    });
}

#[test]
fn dragged_away_it_gets_its_own_size_back_under_the_pointer() {
    let mut h = Headless::new(true);
    one_window(&mut h, |s| {
        super_drag(s, BTN_LEFT, (100.0, 100.0), (0.0, 400.0));
        // Grabbed 292px into the 708px half: the same share of its own
        // 400px width stays under the pointer. (The client hasn't drawn at
        // the half's size, so it's grabbed where it has drawn.)
        let preview = super_drag(s, BTN_LEFT, (300.0, 200.0), (700.0, 200.0));
        assert_eq!(preview, None);
        assert_eq!(where_(s), Some((535, 34, 400, 300)));
        assert!(!told_tiled(&the_window(s)));
        assert!(s.snap_of(&the_window(s)).is_none());
    });
}

#[test]
fn corners_give_quarters() {
    let mut h = Headless::new(true);
    one_window(&mut h, |s| {
        // Along the left edge near the top, and along the bottom near the
        // left.
        assert_eq!(super_drag(s, BTN_LEFT, (100.0, 100.0), (0.0, 30.0)), Some(TOP_LEFT));
        assert_eq!(where_(s), Some(TOP_LEFT));
        assert_eq!(super_drag(s, BTN_LEFT, (100.0, 100.0), (40.0, 807.0)), Some(BOTTOM_LEFT));
        assert_eq!(where_(s), Some(BOTTOM_LEFT));
    });
}

#[test]
fn the_top_edge_maximizes_and_restore_gives_its_own_size() {
    let mut h = Headless::new(true);
    one_window(&mut h, |s| {
        assert_eq!(super_drag(s, BTN_LEFT, (100.0, 100.0), (300.0, 0.0)), Some(USABLE));
        assert!(maximized(s));
        assert_eq!(where_(s), Some(USABLE));
        let w = the_window(s);
        s.unmaximize(&w);
        assert_eq!(where_(s), Some(OWN), "where the drag started");
    });
}

#[test]
fn the_bottom_edge_and_the_middle_dont_snap() {
    let mut h = Headless::new(true);
    one_window(&mut h, |s| {
        assert_eq!(super_drag(s, BTN_LEFT, (100.0, 100.0), (700.0, 807.0)), None);
        assert_eq!(super_drag(s, BTN_LEFT, (100.0, 100.0), (700.0, 400.0)), None);
        assert!(s.snap_of(&the_window(s)).is_none());
    });
}

#[test]
fn snapping_can_be_turned_off() {
    let mut h = Headless::new(true);
    set(&mut h.state, "windows.snap", json!(false));
    one_window(&mut h, |s| {
        assert_eq!(super_drag(s, BTN_LEFT, (100.0, 100.0), (0.0, 400.0)), None);
        assert!(s.snap_of(&the_window(s)).is_none());
        // The arrows move it a step instead.
        arrow(s, KEY_RIGHT);
        assert!(s.snap_of(&the_window(s)).is_none());
    });
}

#[test]
fn snapped_windows_follow_the_gaps_and_survive_maximize() {
    let mut h = Headless::new(true);
    one_window(&mut h, |s| {
        super_drag(s, BTN_LEFT, (100.0, 100.0), (0.0, 400.0));
        set(s, "windows.gaps.outer", json!(0));
        set(s, "windows.gaps.inner", json!(0));
        assert_eq!(where_(s), Some((0, 26, 720, 782)));
        let w = the_window(s);
        s.maximize(&w);
        assert_eq!(where_(s), Some(USABLE));
        s.unmaximize(&w);
        assert_eq!(where_(s), Some((0, 26, 720, 782)), "back in its half");
    });
}

#[test]
fn tiling_or_moving_workspace_ends_the_snap() {
    let mut h = Headless::new(true);
    one_window(&mut h, |s| {
        super_drag(s, BTN_LEFT, (100.0, 100.0), (0.0, 400.0));
        set(s, "windows.layout", json!("dwindle"));
        let w = the_window(s);
        assert!(s.is_tiled(&w) && s.snap_of(&w).is_none());
        // Floating again: where it was before it snapped, at its own size.
        set(s, "windows.layout", json!("floating"));
        assert_eq!(where_(s), Some(OWN));

        super_drag(s, BTN_LEFT, (100.0, 100.0), (0.0, 400.0));
        s.move_focused_to(1);
        assert!(s.snap_of(&w).is_none());
        assert!(!told_tiled(&w));
    });
}

#[test]
fn arrows_step_between_snaps() {
    let mut h = Headless::new(true);
    one_window(&mut h, |s| {
        arrow(s, KEY_LEFT);
        assert_eq!(where_(s), Some(LEFT));
        arrow(s, KEY_UP);
        assert_eq!(where_(s), Some(TOP_LEFT));
        arrow(s, KEY_DOWN);
        assert_eq!(where_(s), Some(LEFT));
        arrow(s, KEY_DOWN);
        assert_eq!(where_(s), Some(BOTTOM_LEFT));
        arrow(s, KEY_DOWN);
        assert_eq!(where_(s), Some(OWN), "its own size, where it was");
        arrow(s, KEY_DOWN);
        assert_eq!(where_(s), Some(OWN), "down never minimises");

        arrow(s, KEY_UP);
        assert!(maximized(s));
        arrow(s, KEY_DOWN);
        assert!(!maximized(s));
        assert_eq!(where_(s).map(|r| (r.2, r.3)), Some((400, 300)));

        arrow(s, KEY_RIGHT);
        assert_eq!(where_(s), Some(RIGHT));
        // Towards the other side: its own size again.
        arrow(s, KEY_LEFT);
        assert_eq!(where_(s).map(|r| (r.2, r.3)), Some((400, 300)));
        assert!(s.snap_of(&the_window(s)).is_none());
    });
}

#[test]
fn a_maximized_window_snaps_and_keeps_its_own_size() {
    let mut h = Headless::new(true);
    one_window(&mut h, |s| {
        arrow(s, KEY_UP);
        arrow(s, KEY_LEFT);
        assert!(!maximized(s));
        assert_eq!(where_(s), Some(LEFT));
        arrow(s, KEY_RIGHT);
        assert_eq!(where_(s), Some(OWN));
    });
}

#[test]
fn arrows_carry_on_to_the_next_monitor_and_its_edge_doesnt_snap() {
    let mut h = Headless::new(true);
    h.add_output("TEST-2", (W, 0));
    one_window(&mut h, |s| {
        // The edge between the two monitors lets windows across.
        assert_eq!(super_drag(s, BTN_LEFT, (100.0, 100.0), (1439.0, 400.0)), None);
        super_drag(s, BTN_LEFT, (1539.0, 400.0), (100.0, 100.0));

        arrow(s, KEY_RIGHT);
        assert_eq!(where_(s), Some(RIGHT));
        arrow(s, KEY_RIGHT);
        // The second monitor's left half: no bar there.
        assert_eq!(where_(s), Some((W + 8, 8, 708, H - 16)));
        // And it stays there through a refit.
        s.retile();
        assert_eq!(where_(s), Some((W + 8, 8, 708, H - 16)));
        // Its far right edge has nothing beyond it.
        arrow(s, KEY_RIGHT);
        assert_eq!(where_(s).map(|r| (r.2, r.3)), Some((400, 300)), "the other side: its own size");
    });
}

/// A bar and two 400x300 windows snapped to the halves (by keys), then
/// drawn at their halves' size so the pointer finds them.
fn two_halves(h: &mut Headless, test: impl FnOnce(&mut WhiteRose) + Send + 'static) {
    h.drive(move |conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let (a, _ta) = open(&mut c);
        c.attach_buffer(&a, 400, 300);
        ask.run(|s| arrow(s, KEY_LEFT));
        let (b, _tb) = open(&mut c);
        c.attach_buffer(&b, 400, 300);
        ask.run(|s| arrow(s, KEY_RIGHT));
        c.attach_buffer(&a, LEFT.2, LEFT.3);
        c.attach_buffer(&b, RIGHT.2, RIGHT.3);
        ask.run(move |s| {
            assert_eq!(rects(s), vec![Some(LEFT), Some(RIGHT)]);
            test(s);
        });
    });
}

fn all_snapped(s: &WhiteRose) -> bool {
    all(s).iter().all(|w| s.snap_of(w).is_some())
}

#[test]
fn dragging_the_gap_between_halves_resizes_both() {
    let mut h = Headless::new(true);
    two_halves(&mut h, |s| {
        drag(s, BTN_LEFT, (720.0, 400.0), (820.0, 400.0));
        assert_eq!(rects(s), vec![Some((8, 34, 808, 766)), Some((824, 34, 608, 766))]);
        assert!(all_snapped(s), "they stay snapped");
        // Refits keep the new split.
        s.retile();
        assert_eq!(rects(s), vec![Some((8, 34, 808, 766)), Some((824, 34, 608, 766))]);
    });
}

#[test]
fn super_right_drag_moves_the_shared_edge_too() {
    let mut h = Headless::new(true);
    two_halves(&mut h, |s| {
        // Nearest the lower-right corner: the bottom is the area's border
        // and stays; the right edge is shared.
        super_drag(s, BTN_RIGHT, (600.0, 600.0), (700.0, 650.0));
        assert_eq!(rects(s), vec![Some((8, 34, 808, 766)), Some((824, 34, 608, 766))]);
    });
}

#[test]
fn no_window_gets_smaller_than_a_tenth() {
    let mut h = Headless::new(true);
    two_halves(&mut h, |s| {
        drag(s, BTN_LEFT, (720.0, 400.0), (1439.0, 400.0));
        assert_eq!(rects(s), vec![Some((8, 34, 1278, 766)), Some((1294, 34, 138, 766))]);
    });
}

#[test]
fn the_middle_of_four_quarters_moves_all_four() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let mut keep = Vec::new();
        for keys in [[KEY_LEFT, KEY_UP], [KEY_RIGHT, KEY_UP], [KEY_LEFT, KEY_DOWN], [KEY_RIGHT, KEY_DOWN]] {
            let (w, t) = open(&mut c);
            c.attach_buffer(&w, 400, 300);
            ask.run(move |s| {
                arrow(s, keys[0]);
                arrow(s, keys[1]);
            });
            keep.push((w, t));
        }
        ask.run(|s| {
            assert_eq!(rects(s)[0], Some(TOP_LEFT));
            assert_eq!(rects(s)[3], Some((724, 421, 708, 379)));
            // The upright line, between the top two.
            drag(s, BTN_LEFT, (720.0, 200.0), (620.0, 200.0));
            assert_eq!(
                rects(s),
                vec![
                    Some((8, 34, 608, 379)),
                    Some((624, 34, 808, 379)),
                    Some((8, 421, 608, 379)),
                    Some((624, 421, 808, 379)),
                ]
            );
        });
    });
}

#[test]
fn a_lone_half_resizes_and_stays_snapped() {
    let mut h = Headless::new(true);
    one_window(&mut h, |s| {
        arrow(s, KEY_LEFT);
        drag(s, BTN_LEFT, (720.0, 400.0), (920.0, 400.0));
        assert_eq!(where_(s), Some((8, 34, 908, 766)));
        assert!(all_snapped(s));
        // The outer gap at the screen's edge moves nothing.
        drag(s, BTN_LEFT, (4.0, 400.0), (300.0, 400.0));
        assert_eq!(where_(s), Some((8, 34, 908, 766)));
    });
}

#[test]
fn windows_on_other_workspaces_dont_follow() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let (a, _ta) = open(&mut c);
        c.attach_buffer(&a, 400, 300);
        ask.run(|s| {
            arrow(s, KEY_LEFT);
            s.switch_workspace(1);
        });
        let (b, _tb) = open(&mut c);
        c.attach_buffer(&b, 400, 300);
        ask.run(|s| {
            arrow(s, KEY_RIGHT);
            drag(s, BTN_LEFT, (720.0, 400.0), (620.0, 400.0));
            assert_eq!(rects(s)[1], Some((624, 34, 808, 766)));
            s.switch_workspace(0);
            assert_eq!(rects(s)[0], Some(LEFT), "the hidden half kept its size");
        });
    });
}

/// The default grid's two zones: two thirds and a third.
const TWO_THIRDS: (i32, i32, i32, i32) = (8, 34, 945, 766);
const LAST_THIRD: (i32, i32, i32, i32) = (961, 34, 471, 766);

fn shift(s: &mut WhiteRose, down: bool) {
    s.on_key(Keycode::new(SHIFT), if down { KeyState::Pressed } else { KeyState::Released }, 0);
}

fn grid(s: &WhiteRose) -> Vec<(i32, i32, i32, i32)> {
    s.snap.grid.iter().map(|(r, _)| (r.loc.x, r.loc.y, r.size.w, r.size.h)).collect()
}

/// Names the test output, so it can have its own grid.
fn name_output(h: &Headless) {
    let o = h.output.as_ref().unwrap();
    whiterose_wm::outputs::set_info(
        o,
        whiterose_wm::outputs::OutputInfo { id: "connector:TEST-1".into(), ..Default::default() },
    );
}

#[test]
fn shift_drag_drops_into_the_default_zones() {
    let mut h = Headless::new(true);
    one_window(&mut h, |s| {
        shift(s, true);
        let preview = super_drag(s, BTN_LEFT, (100.0, 100.0), (1200.0, 400.0));
        shift(s, false);
        assert_eq!(preview, Some(LAST_THIRD), "the zone under the pointer");
        assert_eq!(where_(s), Some(LAST_THIRD));
        assert!(s.snap.grid.is_empty(), "the grid goes when it's dropped");

        // Zones replace the edges while SHIFT is held: the left edge is in
        // the two-thirds zone, not the left half.
        shift(s, true);
        super_drag(s, BTN_LEFT, (1000.0, 100.0), (0.0, 400.0));
        shift(s, false);
        assert_eq!(where_(s), Some(TWO_THIRDS));
    });
}

#[test]
fn shift_shows_and_hides_the_zones_mid_drag() {
    let mut h = Headless::new(true);
    one_window(&mut h, |s| {
        super_key(s, true);
        s.on_pointer_motion((100.0, 100.0).into(), 1);
        s.on_pointer_button(BTN_LEFT, smithay::backend::input::ButtonState::Pressed, 2);
        s.on_pointer_motion((1200.0, 400.0).into(), 3);
        assert_eq!(s.tiling.preview.rect, None);
        assert!(grid(s).is_empty());
        shift(s, true);
        assert_eq!(grid(s), vec![TWO_THIRDS, LAST_THIRD]);
        assert!(s.tiling.preview.rect.is_some());
        shift(s, false);
        assert!(grid(s).is_empty());
        assert_eq!(s.tiling.preview.rect, None);
        s.on_pointer_button(BTN_LEFT, smithay::backend::input::ButtonState::Released, 4);
        super_key(s, false);
        assert!(s.snap_of(&the_window(s)).is_none());
    });
}

#[test]
fn a_monitor_uses_its_own_grid() {
    let mut h = Headless::new(true);
    name_output(&h);
    set(
        &mut h.state,
        "windows.snap.zones",
        json!({"connector:TEST-1": [[0, 0, 0.5, 0.5], [0.5, 0, 0.5, 0.5], [0, 0.5, 1, 0.5]]}),
    );
    one_window(&mut h, |s| {
        shift(s, true);
        let preview = super_drag(s, BTN_LEFT, (100.0, 100.0), (300.0, 600.0));
        shift(s, false);
        assert_eq!(preview, Some((8, 421, 1424, 379)), "the bottom zone");
        assert_eq!(where_(s), Some((8, 421, 1424, 379)));
    });
}

#[test]
fn a_workspace_uses_its_own_grid_over_the_monitors() {
    let mut h = Headless::new(true);
    name_output(&h);
    set(
        &mut h.state,
        "windows.snap.zones",
        json!({"connector:TEST-1": [[0, 0, 0.5, 0.5], [0.5, 0, 0.5, 0.5], [0, 0.5, 1, 0.5]]}),
    );
    // Workspace 2's own grid leaves workspace 1 (the window's) alone.
    set(&mut h.state, "windows.snap.zones.workspaces", json!({"2": {"connector:TEST-1": [[0, 0, 1, 1]]}}));
    one_window(&mut h, |s| {
        shift(s, true);
        super_drag(s, BTN_LEFT, (100.0, 100.0), (300.0, 600.0));
        shift(s, false);
        assert_eq!(where_(s), Some((8, 421, 1424, 379)), "the monitor's bottom zone");
        // Workspace 1's own: one zone, the whole area.
        set(s, "windows.snap.zones.workspaces", json!({"1": {"connector:TEST-1": [[0, 0, 1, 1]]}}));
        shift(s, true);
        let preview = super_drag(s, BTN_LEFT, (300.0, 600.0), (300.0, 200.0));
        shift(s, false);
        assert_eq!(preview, Some((8, 34, 1424, 766)));
    });
}

#[test]
fn a_gap_in_the_grid_drops_nowhere_and_bad_grids_are_ignored() {
    let mut h = Headless::new(true);
    name_output(&h);
    set(&mut h.state, "windows.snap.zones", json!({"connector:TEST-1": [[0, 0, 0.4, 1], [0.6, 0, 0.4, 1]]}));
    one_window(&mut h, |s| {
        shift(s, true);
        let preview = super_drag(s, BTN_LEFT, (100.0, 100.0), (720.0, 400.0));
        assert_eq!(preview, None);
        assert!(s.snap_of(&the_window(s)).is_none());
        // Overlapping zones: the default grid instead.
        set(s, "windows.snap.zones", json!({"connector:TEST-1": [[0, 0, 0.6, 1], [0.5, 0, 0.5, 1]]}));
        // (The first drag left it at (668, 374).)
        let preview = super_drag(s, BTN_LEFT, (700.0, 400.0), (1200.0, 400.0));
        shift(s, false);
        assert_eq!(preview, Some(LAST_THIRD));
    });
}

const Z: u32 = 44 + 8;
const ENTER: u32 = 28 + 8;
const ESC: u32 = 1 + 8;
/// SUPER+Z's picker: five layouts (764 wide) hanging from the bar, in the
/// middle.
const PICKER_AT: (i32, i32) = (338, 26);

fn picker_hovered(s: &WhiteRose) -> Option<(usize, usize)> {
    s.snap.picker.as_ref().and_then(|p| p.hovered)
}

fn preview(s: &WhiteRose) -> Option<(i32, i32, i32, i32)> {
    s.tiling.preview.rect.map(|r| (r.loc.x, r.loc.y, r.size.w, r.size.h))
}

/// A point on the picker opened by SUPER+Z (relative to it).
fn on_picker(x: i32, y: i32) -> (f64, f64) {
    ((PICKER_AT.0 + x) as f64, (PICKER_AT.1 + y) as f64)
}

#[test]
fn super_z_opens_the_picker_and_the_arrows_choose() {
    let mut h = Headless::new(true);
    one_window(&mut h, |s| {
        press(s, &[SUPER, Z]);
        let p = s.snap.picker.as_ref().expect("open");
        assert_eq!((p.at.x, p.at.y), PICKER_AT);
        assert_eq!(picker_hovered(s), Some((0, 0)), "the left half to start with");
        assert_eq!(preview(s), Some(LEFT), "its place shows on the desktop");
        // What the shell draws.
        let whiterose_wm::ipc::Event::SnapPicker { picker: Some(v) } = s.picker_event() else { panic!() };
        assert_eq!((v.x, v.y, v.width, v.height, v.card), (338, 26, 764, 152, false));
        assert_eq!(v.layouts.len(), 5);
        assert_eq!(v.layouts[0].cells, vec![[30, 58, 58, 68], [92, 58, 58, 68]]);
        assert_eq!(v.hovered, Some([0, 0]));

        press(s, &[KEY_RIGHT]);
        assert_eq!(preview(s), Some(RIGHT));
        press(s, &[ENTER]);
        assert!(s.snap.picker.is_none());
        assert_eq!(where_(s), Some(RIGHT));
        assert_eq!(preview(s), None);
    });
}

#[test]
fn escape_or_super_z_again_closes_it() {
    let mut h = Headless::new(true);
    one_window(&mut h, |s| {
        press(s, &[SUPER, Z]);
        press(s, &[ESC]);
        assert!(s.snap.picker.is_none());
        assert_eq!(preview(s), None);
        press(s, &[SUPER, Z]);
        press(s, &[SUPER, Z]);
        assert!(s.snap.picker.is_none());
        assert!(s.snap_of(&the_window(s)).is_none(), "nothing chosen");
    });
}

#[test]
fn the_pointer_hovers_and_a_click_chooses() {
    let mut h = Headless::new(true);
    one_window(&mut h, |s| {
        press(s, &[SUPER, Z]);
        // The first of the thirds.
        s.on_pointer_motion(on_picker(182, 60).into(), 1);
        assert_eq!(picker_hovered(s), Some((1, 0)));
        s.on_pointer_button(BTN_LEFT, smithay::backend::input::ButtonState::Pressed, 2);
        s.on_pointer_button(BTN_LEFT, smithay::backend::input::ButtonState::Released, 3);
        assert!(s.snap.picker.is_none());
        assert_eq!(where_(s), Some((8, 34, 471, 766)));
    });
}

#[test]
fn a_click_outside_closes_it() {
    let mut h = Headless::new(true);
    one_window(&mut h, |s| {
        press(s, &[SUPER, Z]);
        s.on_pointer_motion((700.0, 600.0).into(), 1);
        assert_eq!(picker_hovered(s), Some((0, 0)), "moving off it keeps the arrows' choice");
        s.on_pointer_button(BTN_LEFT, smithay::backend::input::ButtonState::Pressed, 2);
        s.on_pointer_button(BTN_LEFT, smithay::backend::input::ButtonState::Released, 3);
        assert!(s.snap.picker.is_none());
        assert!(s.snap_of(&the_window(s)).is_none());
    });
}

#[test]
fn dragging_to_the_top_middle_opens_it_to_drop_on() {
    let mut h = Headless::new(true);
    one_window(&mut h, |s| {
        use smithay::backend::input::ButtonState;
        super_key(s, true);
        s.on_pointer_motion((100.0, 100.0).into(), 1);
        s.on_pointer_button(BTN_LEFT, ButtonState::Pressed, 2);
        s.on_pointer_motion((720.0, 0.0).into(), 3);
        let p = s.snap.picker.as_ref().expect("open");
        assert_eq!((p.at.x, p.at.y), PICKER_AT);
        assert_eq!(preview(s), None, "nothing chosen yet");
        // Down onto the quarters' lower right.
        s.on_pointer_motion(on_picker(420, 120).into(), 4);
        assert_eq!(picker_hovered(s), Some((2, 3)));
        assert_eq!(preview(s), Some((724, 421, 708, 379)));
        s.on_pointer_button(BTN_LEFT, ButtonState::Released, 5);
        super_key(s, false);
        assert!(s.snap.picker.is_none());
        assert_eq!(where_(s), Some((724, 421, 708, 379)));
    });
}

#[test]
fn dragging_away_from_the_picker_closes_it() {
    let mut h = Headless::new(true);
    one_window(&mut h, |s| {
        use smithay::backend::input::ButtonState;
        super_key(s, true);
        s.on_pointer_motion((100.0, 100.0).into(), 1);
        s.on_pointer_button(BTN_LEFT, ButtonState::Pressed, 2);
        s.on_pointer_motion((720.0, 0.0).into(), 3);
        assert!(s.snap.picker.is_some());
        s.on_pointer_motion((720.0, 500.0).into(), 4);
        assert!(s.snap.picker.is_none());
        s.on_pointer_button(BTN_LEFT, ButtonState::Released, 5);
        super_key(s, false);
        assert!(s.snap_of(&the_window(s)).is_none());
    });
}

#[test]
fn a_maximize_button_opens_a_card_under_the_pointer() {
    let mut h = Headless::new(true);
    one_window(&mut h, |s| {
        // As a WhiteRose app asks for it: the pointer on its button.
        s.on_pointer_motion((100.0, 90.0).into(), 1);
        let req = whiterose_wm::ipc::Request::parse(r#"{"cmd":"snap_picker","under_pointer":true}"#).unwrap();
        s.run_action(req.into_action());
        let p = s.snap.picker.as_ref().expect("open");
        // Centred under the pointer would be off the left of the screen:
        // kept 8px inside it.
        assert_eq!((p.at.x, p.at.y, p.card), (8, 106, true));
        assert_eq!(picker_hovered(s), None);
        // A little way off (towards it) keeps it; well away closes it.
        s.on_pointer_motion((100.0, 70.0).into(), 2);
        assert!(s.snap.picker.is_some());
        s.on_pointer_motion((1300.0, 700.0).into(), 3);
        assert!(s.snap.picker.is_none());
    });
}

#[test]
fn no_picker_on_tiled_workspaces() {
    let mut h = Headless::new(true);
    two_tiles(&mut h, |s| {
        press(s, &[SUPER, Z]);
        assert!(s.snap.picker.is_none());
    });
}

#[test]
fn super_then_shift_drag_drops_into_a_zone() {
    let mut h = Headless::new(true);
    one_window(&mut h, |s| {
        use smithay::backend::input::ButtonState;
        super_key(s, true);
        shift(s, true);
        s.on_pointer_motion((100.0, 100.0).into(), 1);
        s.on_pointer_button(BTN_LEFT, ButtonState::Pressed, 2);
        for i in 1..=8 {
            let f = i as f64 / 8.0;
            s.on_pointer_motion((100.0 + 1100.0 * f, 100.0 + 300.0 * f).into(), 2 + i);
        }
        s.on_pointer_button(BTN_LEFT, ButtonState::Released, 20);
        shift(s, false);
        super_key(s, false);
        assert_eq!(where_(s), Some(LAST_THIRD));
    });
}
