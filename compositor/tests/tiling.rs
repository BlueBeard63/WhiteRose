//! Tiled workspaces with real clients: windows get tiles from the layout
//! settings, give their space back when they close or minimise, float
//! again where they were, and dialogs and fixed-size windows float.

mod common;

use common::{tiles::*, *};
use serde_json::json;
use whiterose_wm::{keybind::Action, outputs};

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

// SUPER+ALT+arrow: the focused window to the workspace before or after,
// and there with it; nothing at the first or last.
#[test]
fn super_alt_arrows_take_the_window_to_the_next_workspace() {
    use whiterose_wm::layout::Dir;
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        open(&mut c);
        open(&mut c);
        c.queue.roundtrip(&mut c.app).unwrap();
        ask.run(|s| {
            let w = all(s)[1].clone();
            assert_eq!(s.focused_window(), Some(w.clone()));
            s.run_action(Action::MoveToWorkspaceStep(Dir::Left));
            assert_eq!(
                (s.workspaces.active(), s.workspaces.workspace_of(&w)),
                (0, Some(0)),
                "the first: nowhere to go"
            );

            s.run_action(Action::MoveToWorkspaceStep(Dir::Right));
            assert_eq!(s.workspaces.active(), 1, "went with it");
            assert_eq!(s.workspaces.workspace_of(&w), Some(1));
            assert_eq!(s.focused_window(), Some(w.clone()), "still focused");
            assert_eq!(s.workspaces.workspace_of(&all(s)[0]), Some(0), "the other stayed");

            s.run_action(Action::MoveToWorkspaceStep(Dir::Up));
            assert_eq!((s.workspaces.active(), s.workspaces.workspace_of(&w)), (0, Some(0)), "up: back");

            let last = s.workspaces.count() - 1;
            s.run_action(Action::MoveToWorkspace(last as u8 + 1));
            s.switch_workspace(last);
            s.focus_window(Some(w.clone()));
            s.run_action(Action::MoveToWorkspaceStep(Dir::Down));
            assert_eq!(
                (s.workspaces.active(), s.workspaces.workspace_of(&w)),
                (last, Some(last)),
                "the last: nowhere to go"
            );
        });
    });
}

// SUPER+CTRL+arrow: the focused window to the monitor on that side.
// Tiled, it joins the tiles there (the one left behind takes its screen);
// floating, it keeps its place relative to the screen; maximized, it
// fills the other screen. Nothing that way: it stays.
#[test]
fn super_ctrl_arrows_move_the_window_to_the_next_monitor() {
    use whiterose_wm::layout::Dir;
    let mut h = Headless::new(true);
    let second = h.add_output("TEST-2", (W, 0));
    set(&mut h.state, "windows.layout", json!("dwindle"));
    h.drive(move |conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        open(&mut c);
        open(&mut c);
        c.queue.roundtrip(&mut c.app).unwrap();
        ask.run(move |s| {
            let first = s.output_of(&all(s)[0]).unwrap();
            assert_eq!(rects(s), vec![Some(LEFT), Some(RIGHT)]);
            let w = all(s)[1].clone();
            s.run_action(Action::MoveToOutput(Dir::Left));
            assert_eq!(s.output_of(&w), Some(first.clone()), "nothing to the left");

            s.run_action(Action::MoveToOutput(Dir::Right));
            assert_eq!(s.output_of(&w), Some(second.clone()), "on the other monitor");
            assert!(s.is_tiled(&w), "in its tiles");
            assert!(rects(s)[1].unwrap().0 >= W, "drawn there: {:?}", rects(s));
            assert_eq!(rects(s)[0], Some(AREA), "the one left behind takes its screen");
            assert_eq!(s.focused_window(), Some(w.clone()), "still focused");

            s.run_action(Action::MoveToOutput(Dir::Left));
            assert_eq!(s.output_of(&w), Some(first.clone()), "and back");
            assert_eq!(rects(s), vec![Some(LEFT), Some(RIGHT)]);
        });
    });
}

#[test]
fn floating_and_maximized_windows_move_to_the_next_monitor_too() {
    use whiterose_wm::layout::Dir;
    let mut h = Headless::new(true);
    let second = h.add_output("TEST-2", (W, 0));
    h.drive(move |conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let (surface, _t) = open(&mut c);
        c.attach_buffer(&surface, 400, 300);
        let (surface, _t) = open(&mut c);
        c.attach_buffer(&surface, 400, 300);
        c.queue.roundtrip(&mut c.app).unwrap();
        ask.run(move |s| {
            let (a, b) = (all(s)[0].clone(), all(s)[1].clone());
            let before = s.space.element_location(&b).unwrap();
            s.focus_window(Some(b.clone()));
            s.run_action(Action::MoveToOutput(Dir::Right));
            let after = s.space.element_location(&b).unwrap();
            assert_eq!(s.output_of(&b), Some(second.clone()));
            assert_eq!(after.x - W, before.x, "the same place on a screen the same size");

            s.maximize(&a);
            s.focus_window(Some(a.clone()));
            s.run_action(Action::MoveToOutput(Dir::Right));
            assert_eq!(s.output_of(&a), Some(second.clone()));
            let area = s.usable_area_on(&second);
            assert_eq!(s.space.element_location(&a), Some(area.loc), "fills it");
            let size = a.toplevel().unwrap().with_pending_state(|p| p.size);
            assert_eq!(size, Some(area.size));
        });
    });
}
