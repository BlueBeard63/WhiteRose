//! Layer-shell behaviour QuickShell depends on, tested with a real client
//! against a headless compositor with a fake 1440x808 output.

mod common;

use common::*;
use smithay::desktop::layer_map_for_output;
use wayland_protocols_wlr::layer_shell::v1::client::{zwlr_layer_shell_v1, zwlr_layer_surface_v1};

// ---------------------------------------------------------------- tests

#[test]
fn bar_spans_the_output_and_reserves_its_height() {
    let mut h = Headless::new(true);
    h.run(
        |conn| {
            let mut c = Client::connect(&conn);
            let _bar = c.open_bar();
            (c.app.layer_configure, c.app.layer_closed)
        },
        |state, (configure, closed)| {
            assert!(!closed, "bar was closed");
            assert_eq!(configure, Some((W as u32, BAR)), "bar should be stretched to the output width");
            let area = state.usable_area();
            assert_eq!(area.loc.y, BAR as i32, "windows must start below the bar");
            assert_eq!(area.size.h, H - BAR as i32);
            assert_eq!(area.size.w, W);
        },
    );
}

#[test]
fn new_windows_open_below_the_bar() {
    let mut h = Headless::new(true);
    h.run(
        |conn| {
            let mut c = Client::connect(&conn);
            let _bar = c.open_bar();
            let qh = c.queue.handle();
            let surface = c.compositor.create_surface(&qh, ());
            let xdg_surface = c.xdg.get_xdg_surface(&surface, &qh, ());
            let _toplevel = xdg_surface.get_toplevel(&qh, ());
            surface.commit();
            c.until(|a| a.toplevel_configured);
        },
        |state, ()| {
            let windows: Vec<_> = state.space.elements().cloned().collect();
            assert_eq!(windows.len(), 1);
            let loc = state.space.element_location(&windows[0]).unwrap();
            assert!(loc.y >= BAR as i32, "window placed under the bar at {loc:?}");
            assert_eq!(state.workspaces.active_windows().len(), 1);
            assert!(state.focused_window().is_some(), "new window should be focused");
        },
    );
}

#[test]
fn exclusive_overlay_takes_the_keyboard() {
    // The app launcher (SUPER) and screenshot overlay must get typing focus.
    let mut h = Headless::new(true);
    h.run(
        |conn| {
            let mut c = Client::connect(&conn);
            let qh = c.queue.handle();
            let surface = c.compositor.create_surface(&qh, ());
            let layer = c.layer_shell.get_layer_surface(
                &surface,
                None,
                zwlr_layer_shell_v1::Layer::Overlay,
                "whiterose-launcher".into(),
                &qh,
                (),
            );
            layer.set_size(640, 480);
            layer.set_anchor(zwlr_layer_surface_v1::Anchor::Top);
            layer.set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive);
            surface.commit();
            c.until(|a| a.layer_configure.is_some());
            // Commit again after acking, as a real client would before drawing.
            surface.commit();
            c.queue.roundtrip(&mut c.app).unwrap();
            c.app.layer_configure
        },
        |state, configure| {
            assert_eq!(configure, Some((640, 480)));
            let output = state.space.outputs().next().unwrap().clone();
            let map = layer_map_for_output(&output);
            let launcher = map.layers().find(|l| l.namespace() == "whiterose-launcher").expect("launcher mapped");
            assert_eq!(
                state.keyboard_focus().as_ref(),
                Some(launcher.wl_surface()),
                "launcher should have keyboard focus"
            );
        },
    );
}

#[test]
fn layer_surface_without_any_output_is_closed() {
    let mut h = Headless::new(false);
    assert!(h.output.is_none());
    h.run(
        |conn| {
            let mut c = Client::connect(&conn);
            c.open_bar();
            c.app.layer_closed
        },
        |_, closed| assert!(closed, "client must be told when its panel can't be shown"),
    );
}

#[test]
fn exclusive_focus_is_granted_once_not_stolen_back() {
    use std::io::{BufRead, BufReader, Write};
    let mut h = Headless::new(true);
    let ipc_path = h.state.ipc.as_ref().unwrap().path.clone();
    h.run(
        move |conn| {
            let ipc = std::os::unix::net::UnixStream::connect(&ipc_path).unwrap();
            ipc.set_read_timeout(Some(std::time::Duration::from_secs(3))).unwrap();
            let mut ipc_w = ipc.try_clone().unwrap();
            let mut ipc_r = BufReader::new(ipc);

            let mut c = Client::connect(&conn);
            let (_win, _t) = c.open_window();
            // The window's id, from the compositor's window list.
            let id = loop {
                let mut line = String::new();
                ipc_r.read_line(&mut line).unwrap();
                let v: serde_json::Value = serde_json::from_str(&line).unwrap();
                if let Some(id) = v["windows"][0]["id"].as_u64() {
                    break id;
                }
            };

            // The dock opens and takes the keyboard...
            let qh = c.queue.handle();
            let surface = c.compositor.create_surface(&qh, ());
            let layer = c.layer_shell.get_layer_surface(
                &surface,
                None,
                zwlr_layer_shell_v1::Layer::Top,
                "whiterose-dock".into(),
                &qh,
                (),
            );
            layer.set_size(400, 90);
            layer.set_anchor(zwlr_layer_surface_v1::Anchor::Top);
            layer.set_keyboard_interactivity(zwlr_layer_surface_v1::KeyboardInteractivity::Exclusive);
            surface.commit();
            c.until(|a| a.layer_configure.is_some());
            c.attach_buffer(&surface, 400, 90);

            // ...then the user clicks the window (here: focus it over IPC)...
            ipc_w.write_all(format!("{{\"cmd\":\"focus_window\",\"id\":{id}}}\n").as_bytes()).unwrap();
            c.queue.roundtrip(&mut c.app).unwrap();
            // ...and the dock keeps committing frames: it must not steal focus back.
            for _ in 0..3 {
                surface.damage_buffer(0, 0, 1, 1);
                surface.commit();
                c.queue.roundtrip(&mut c.app).unwrap();
            }
            id
        },
        |state, id| {
            let w = state.focused_window().unwrap();
            assert_eq!(state.window_id(&w), id);
            assert_eq!(
                state.keyboard_focus().as_ref(),
                Some(w.toplevel().unwrap().wl_surface()),
                "a committing exclusive layer stole the keyboard back from the window"
            );
        },
    );
}
