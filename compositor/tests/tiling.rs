//! Tiled workspaces with real clients: windows get tiles from the layout
//! settings, give their space back when they close or minimise, float
//! again where they were, and dialogs and fixed-size windows float.

mod common;

use common::*;
use serde_json::{json, Value};
use smithay::{desktop::Window, reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as ServerState};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_protocols::xdg::shell::client::xdg_toplevel::XdgToplevel;
use whiterose_wm::{keybind::Action, outputs, WhiteRose};

/// The area below the bar, less the 8px outer gap.
const AREA: (i32, i32, i32, i32) = (8, 34, 1424, 766);
/// Its two halves, with the 8px gap between them.
const LEFT: (i32, i32, i32, i32) = (8, 34, 708, 766);
const RIGHT: (i32, i32, i32, i32) = (724, 34, 708, 766);

fn set(state: &mut WhiteRose, key: &str, value: Value) {
    if state.layout_setting(key, &value) {
        state.apply_layout_settings();
    }
}

/// Every window, oldest first.
fn all(state: &WhiteRose) -> Vec<Window> {
    let mut ws: Vec<Window> = state.workspaces.mru().to_vec();
    ws.sort_by_key(|w| state.window_id(w));
    ws
}

/// Where a window is and the size it was told to take (0x0: its own
/// choice); None while it's hidden.
fn rect(state: &WhiteRose, w: &Window) -> Option<(i32, i32, i32, i32)> {
    let loc = state.space.element_location(w)?;
    let size = w.toplevel().unwrap().with_pending_state(|s| s.size).unwrap_or_default();
    Some((loc.x, loc.y, size.w, size.h))
}

fn rects(state: &WhiteRose) -> Vec<Option<(i32, i32, i32, i32)>> {
    all(state).iter().map(|w| rect(state, w)).collect()
}

fn told_tiled(w: &Window) -> bool {
    w.toplevel().unwrap().with_pending_state(|s| s.states.contains(ServerState::TiledLeft))
}

/// Opens a window, letting `setup` describe it before its first commit.
fn open_with(c: &mut Client, setup: impl FnOnce(&XdgToplevel)) -> (WlSurface, XdgToplevel) {
    let qh = c.queue.handle();
    let surface = c.compositor.create_surface(&qh, ());
    let xdg_surface = c.xdg.get_xdg_surface(&surface, &qh, ());
    let toplevel = xdg_surface.get_toplevel(&qh, ());
    setup(&toplevel);
    surface.commit();
    c.app.toplevel_configured = false;
    c.until(|a| a.toplevel_configured);
    (surface, toplevel)
}

fn open(c: &mut Client) -> (WlSurface, XdgToplevel) {
    open_with(c, |_| {})
}

#[test]
fn floating_is_the_default() {
    let mut h = Headless::new(true);
    h.run(
        |conn| {
            let mut c = Client::connect(&conn);
            c.open_bar();
            open(&mut c);
            open(&mut c);
            c.queue.roundtrip(&mut c.app).unwrap();
        },
        |state, ()| {
            // The cascade, below the bar, sizes left to the apps.
            assert_eq!(rects(state), vec![Some((48, 74, 0, 0)), Some((80, 106, 0, 0))]);
            assert!(state.tiling.sets.is_empty());
            assert!(!all(state).iter().any(told_tiled));
        },
    );
}

#[test]
fn dwindle_tiles_new_windows_below_the_bar() {
    let mut h = Headless::new(true);
    set(&mut h.state, "windows.layout", json!("dwindle"));
    h.run(
        |conn| {
            let mut c = Client::connect(&conn);
            c.open_bar();
            open(&mut c);
            open(&mut c);
            c.queue.roundtrip(&mut c.app).unwrap();
            c.app.toplevel_configures.last().cloned().unwrap()
        },
        |state, (w, h, states)| {
            assert_eq!(rects(state), vec![Some(LEFT), Some(RIGHT)]);
            // The client heard it: its size, and that it's tiled on every side.
            assert_eq!((w, h), (708, 766));
            for tiled in 5..=8 {
                assert!(states.contains(&tiled), "states: {states:?}");
            }
        },
    );
}

#[test]
fn master_puts_the_first_window_on_the_left() {
    let mut h = Headless::new(true);
    set(&mut h.state, "windows.layout", json!("master"));
    set(&mut h.state, "windows.gaps.inner", json!(0));
    set(&mut h.state, "windows.gaps.outer", json!(0));
    set(&mut h.state, "windows.master.ratio", json!(0.5));
    h.run(
        |conn| {
            let mut c = Client::connect(&conn);
            c.open_bar();
            for _ in 0..3 {
                open(&mut c);
            }
            c.queue.roundtrip(&mut c.app).unwrap();
        },
        |state, ()| {
            assert_eq!(
                rects(state),
                vec![Some((0, 26, 720, 782)), Some((720, 26, 720, 391)), Some((720, 417, 720, 391))]
            );
            // Settings apply live: the master moves to the right.
            set(state, "windows.master.side", json!("right"));
            assert_eq!(rects(state)[0], Some((720, 26, 720, 782)));
        },
    );
}

#[test]
fn turning_tiling_off_puts_windows_back_where_they_floated() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let (surface, _t) = open(&mut c);
        c.attach_buffer(&surface, 400, 300);

        ask.run(|s| {
            set(s, "windows.layout", json!("dwindle"));
            assert_eq!(rects(s), vec![Some(AREA)]);
            assert!(told_tiled(&all(s)[0]));
        });
        ask.run(|s| {
            set(s, "windows.layout", json!("floating"));
            assert_eq!(rects(s), vec![Some((48, 74, 400, 300))]);
            assert!(!told_tiled(&all(s)[0]));
            assert!(s.tiling.floating_at.is_empty());
        });
    });
}

#[test]
fn a_workspace_tiles_on_its_own_and_moved_windows_float_again() {
    let mut h = Headless::new(true);
    set(&mut h.state, "windows.layout.workspaces", json!({"2": "dwindle"}));
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        open(&mut c);
        ask.run(|s| s.switch_workspace(1));
        open(&mut c);
        open(&mut c);
        c.queue.roundtrip(&mut c.app).unwrap();

        ask.run(|s| {
            assert_eq!(rects(s), vec![None, Some(LEFT), Some(RIGHT)], "workspace 1 stays floating");
            // The newest window goes to workspace 1; the other takes its space.
            s.run_action(Action::MoveToWorkspace(1));
            assert_eq!(rects(s), vec![None, Some(AREA), None]);
            s.switch_workspace(0);
            // It floats where it opened (second in the cascade on workspace 2).
            assert_eq!(rects(s), vec![Some((48, 74, 0, 0)), None, Some((80, 106, 0, 0))]);
            assert!(!told_tiled(&all(s)[2]));
        });
    });
}

#[test]
fn closing_and_minimising_give_the_space_back() {
    let mut h = Headless::new(true);
    set(&mut h.state, "windows.layout", json!("dwindle"));
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        open(&mut c);
        let (surface, toplevel) = open(&mut c);
        c.queue.roundtrip(&mut c.app).unwrap();

        ask.run(|s| {
            let second = all(s)[1].clone();
            s.minimize(&second);
            assert_eq!(rects(s), vec![Some(AREA), None]);
            // Back from the dock: it splits the tile again.
            s.focus_window(Some(second));
            assert_eq!(rects(s), vec![Some(LEFT), Some(RIGHT)]);
        });
        toplevel.destroy();
        surface.destroy();
        c.queue.roundtrip(&mut c.app).unwrap();
        ask.run(|s| assert_eq!(rects(s), vec![Some(AREA)]));
    });
}

#[test]
fn dialogs_and_fixed_size_windows_float_above_the_tiles() {
    let mut h = Headless::new(true);
    set(&mut h.state, "windows.layout", json!("dwindle"));
    h.run(
        |conn| {
            let mut c = Client::connect(&conn);
            c.open_bar();
            let (_s, main) = open(&mut c);
            open_with(&mut c, |t| t.set_parent(Some(&main)));
            open_with(&mut c, |t| {
                t.set_min_size(300, 200);
                t.set_max_size(300, 200);
            });
            c.queue.roundtrip(&mut c.app).unwrap();
        },
        |state, ()| {
            let ws = all(state);
            assert_eq!(rect(state, &ws[0]), Some(AREA), "only the main window is tiled");
            assert!(!state.is_tiled(&ws[1]) && !state.is_tiled(&ws[2]));
            assert!(!told_tiled(&ws[1]) && !told_tiled(&ws[2]));
            // Focusing the tile keeps the floating windows above it.
            state.focus_window(Some(ws[0].clone()));
            assert_eq!(state.space.elements().next(), Some(&ws[0]), "the tile is at the bottom");
        },
    );
}

#[test]
fn a_maximized_tile_returns_to_its_place() {
    let mut h = Headless::new(true);
    set(&mut h.state, "windows.layout", json!("dwindle"));
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let (_s, first) = open(&mut c);
        open(&mut c);
        first.set_maximized();
        c.queue.roundtrip(&mut c.app).unwrap();
        ask.run(|s| {
            assert_eq!(rects(s), vec![Some((0, 26, 1440, 782)), Some(RIGHT)], "keeps its place in the tree");
            let ws = all(s);
            assert_eq!(s.space.elements().last(), Some(&ws[0]), "above the other tiles");
        });
        first.unset_maximized();
        c.queue.roundtrip(&mut c.app).unwrap();
        ask.run(|s| assert_eq!(rects(s), vec![Some(LEFT), Some(RIGHT)]));
    });
}

#[test]
fn tiles_make_room_when_the_bar_appears() {
    let mut h = Headless::new(true);
    set(&mut h.state, "windows.layout", json!("dwindle"));
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        open(&mut c);
        c.queue.roundtrip(&mut c.app).unwrap();
        ask.run(|s| assert_eq!(rects(s), vec![Some((8, 8, 1424, 792))]));
        c.open_bar();
        c.queue.roundtrip(&mut c.app).unwrap();
        ask.run(|s| assert_eq!(rects(s), vec![Some(AREA)]));
    });
}

#[test]
fn a_monitor_setting_beats_the_workspace_and_tiles_only_that_monitor() {
    let mut h = Headless::new(true);
    let second = h.add_output("TEST-2", (W, 0));
    outputs::set_info(&second, outputs::OutputInfo { id: "connector:TEST-2".into(), ..Default::default() });
    set(&mut h.state, "windows.layout.workspaces", json!({"1": "dwindle"}));
    set(&mut h.state, "windows.layout.monitors", json!({"connector:TEST-2": "floating"}));
    let first = h.output.clone().unwrap();
    assert_eq!(h.state.layout_of(0, &first).as_str(), "dwindle");
    assert_eq!(h.state.layout_of(0, &second).as_str(), "floating");
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        open(&mut c);
        // New windows open on the output under the pointer.
        ask.run(|s| s.on_pointer_motion((W as f64 + 100.0, 100.0).into(), 1));
        open(&mut c);
        c.queue.roundtrip(&mut c.app).unwrap();
        ask.run(|s| {
            let ws = all(s);
            assert_eq!(rect(s, &ws[0]), Some((8, 8, 1424, 792)), "tiled on the first monitor");
            assert!(!s.is_tiled(&ws[1]), "floating on the second");
        });
    });
}
