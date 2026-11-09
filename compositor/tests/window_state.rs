//! Maximize and fullscreen, driven by a real client (title-bar buttons send
//! these requests) and by the SUPER+F shortcut.

mod common;

use common::*;
use smithay::{reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as ServerState, utils::Point};
use whiterose_wm::keybind::Action;

#[test]
fn maximize_fills_the_area_below_the_bar_and_restores() {
    let mut h = Headless::new(true);
    h.run(
        |conn| {
            let mut c = Client::connect(&conn);
            let _bar = c.open_bar();
            let (_surface, toplevel) = c.open_window();

            let seen = c.app.toplevel_configures.len();
            toplevel.set_maximized();
            let maximized = c.next_toplevel_configure(seen);

            toplevel.unset_maximized();
            let restored = c.next_toplevel_configure(seen + 1);
            (maximized, restored)
        },
        |_, ((w, h, states), (_, _, restored_states))| {
            assert_eq!((w, h), (W, H - BAR as i32), "maximized size should be the screen minus the bar");
            assert!(states.contains(&MAXIMIZED), "states: {states:?}");
            assert!(!restored_states.contains(&MAXIMIZED), "still maximized after unset: {restored_states:?}");
        },
    );
}

#[test]
fn bar_gets_clicks_on_its_strip_normally() {
    // Control for the fullscreen test below: without fullscreen, the bar wins.
    let mut h = Headless::new(true);
    h.run(
        |conn| {
            let mut c = Client::connect(&conn);
            let (bar, _layer) = c.open_bar();
            c.attach_buffer(&bar, W, BAR as i32);
            let (win, _t) = c.open_window();
            c.attach_buffer(&win, 400, 300);
        },
        |state, ()| {
            let (surface, _) = state.surface_under(Point::from((300.0, 10.0))).expect("bar is under the pointer");
            let output = state.space.outputs().next().unwrap().clone();
            let map = smithay::desktop::layer_map_for_output(&output);
            let bar = map.layers().find(|l| l.namespace() == "whiterose-bar").unwrap();
            assert_eq!(&surface, bar.wl_surface());
        },
    );
}

#[test]
fn fullscreen_covers_the_bar_and_takes_the_pointer() {
    let mut h = Headless::new(true);
    h.run(
        |conn| {
            let mut c = Client::connect(&conn);
            let (bar, _layer) = c.open_bar();
            c.attach_buffer(&bar, W, BAR as i32);
            let (surface, toplevel) = c.open_window();
            let seen = c.app.toplevel_configures.len();
            toplevel.set_fullscreen(None);
            let configure = c.next_toplevel_configure(seen);
            // Draw at the size we were given, as a real app would.
            c.attach_buffer(&surface, configure.0, configure.1);
            configure
        },
        |state, (w, h, states)| {
            assert_eq!((w, h), (W, H), "fullscreen size should be the whole output");
            assert!(states.contains(&FULLSCREEN), "states: {states:?}");

            let fs = state.active_fullscreen().cloned().expect("compositor tracks the fullscreen window");
            assert_eq!(state.space.element_location(&fs), Some(Point::from((0, 0))));
            // A point inside the bar's strip now belongs to the window, not the bar.
            let bar_strip = Point::from((300.0, 10.0));
            let (surface, _) = state.surface_under(bar_strip).expect("something under the pointer");
            assert_eq!(&surface, fs.toplevel().unwrap().wl_surface(), "the bar should be covered");
        },
    );
}

#[test]
fn super_f_toggles_fullscreen_on_the_focused_window() {
    let mut h = Headless::new(true);
    h.run(
        |conn| {
            let mut c = Client::connect(&conn);
            c.open_window();
        },
        |state, ()| {
            let w = state.focused_window().expect("new window is focused");
            let fullscreen = |w: &smithay::desktop::Window| {
                w.toplevel().unwrap().with_pending_state(|s| s.states.contains(ServerState::Fullscreen))
            };

            state.run_action(Action::ToggleFullscreen);
            assert!(fullscreen(&w));
            assert_eq!(state.active_fullscreen(), Some(&w));

            state.run_action(Action::ToggleFullscreen);
            assert!(!fullscreen(&w));
            assert_eq!(state.active_fullscreen(), None);
        },
    );
}

#[test]
fn opening_a_window_leaves_fullscreen() {
    let mut h = Headless::new(true);
    h.run(
        |conn| {
            let mut c = Client::connect(&conn);
            let (_s, first) = c.open_window();
            let seen = c.app.toplevel_configures.len();
            first.set_fullscreen(None);
            c.next_toplevel_configure(seen);
            // A second window, e.g. a dialog: it must not end up hidden behind fullscreen.
            c.app.toplevel_configured = false;
            c.open_window();
            c.queue.roundtrip(&mut c.app).unwrap();
        },
        |state, ()| {
            assert_eq!(state.active_fullscreen(), None, "fullscreen should end when a new window opens");
            assert_eq!(state.space.elements().count(), 2);
        },
    );
}

#[test]
fn closing_a_fullscreen_window_clears_fullscreen() {
    let mut h = Headless::new(true);
    h.run(
        |conn| {
            let mut c = Client::connect(&conn);
            let (surface, toplevel) = c.open_window();
            let seen = c.app.toplevel_configures.len();
            toplevel.set_fullscreen(None);
            c.next_toplevel_configure(seen);
            toplevel.destroy();
            surface.destroy();
            c.queue.roundtrip(&mut c.app).unwrap();
        },
        |state, ()| {
            assert_eq!(state.active_fullscreen(), None);
            assert!(state.fullscreen.is_none(), "stale fullscreen reference kept");
            assert!(state.restore_geometry.is_empty(), "stale restore geometry kept");
        },
    );
}

#[test]
fn minimize_hides_until_focused_again() {
    let mut h = Headless::new(true);
    h.run(
        |conn| {
            let mut c = Client::connect(&conn);
            let (a, _ta) = c.open_window();
            c.attach_buffer(&a, 400, 300);
            let (b, tb) = c.open_window();
            c.attach_buffer(&b, 400, 300);
            // The title bar's minimise button on the newest window.
            tb.set_minimized();
            c.queue.roundtrip(&mut c.app).unwrap();
        },
        |state, ()| {
            let all: Vec<_> = state.workspaces.mru().to_vec();
            assert_eq!(all.len(), 2);
            let hidden = state.minimized.first().cloned().expect("a window is minimised");
            assert_eq!(state.space.elements().count(), 1, "it's gone from the screen");
            let other = all.iter().find(|w| **w != hidden).unwrap().clone();
            assert_eq!(state.focused_window(), Some(other.clone()), "focus moves to the other window");

            // Away and back: it stays minimised.
            state.switch_workspace(1);
            state.switch_workspace(0);
            assert_eq!(state.space.elements().count(), 1, "switching workspaces doesn't bring it back");

            // Clicking it in the dock or ALT+TAB focuses it: it's back.
            state.activate_window(&hidden);
            assert!(state.minimized.is_empty());
            assert_eq!(state.space.elements().count(), 2);
            assert_eq!(state.focused_window(), Some(hidden));
        },
    );
}
