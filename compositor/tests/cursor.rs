//! The cursor over nothing (no window, layer or wallpaper under it).

mod common;

use common::*;
use smithay::input::pointer::{CursorIcon, CursorImageStatus};

/// An app's cursor (a resize arrow at its edge) doesn't stay once the
/// pointer leaves it for nothing, like a gap between windows: it's the
/// arrow again.
#[test]
fn over_nothing_the_cursor_is_the_arrow() {
    let mut h = Headless::new(true);
    h.state.cursor_status = CursorImageStatus::Named(CursorIcon::EwResize);
    h.state.on_pointer_motion((300.0, 300.0).into(), 1);
    assert_eq!(h.state.cursor_status, CursorImageStatus::default_named());
}

/// An app's own cursor surface gets frame callbacks like its windows.
/// Apps that wait for one before drawing their next cursor (GTK, Qt) kept
/// the last one (a resize arrow, a hand) when it changed twice quickly.
#[test]
fn an_apps_cursor_surface_gets_frame_callbacks() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        let qh = c.queue.handle();
        let pointer = c.seat.get_pointer(&qh, ());
        let (a, _ta) = tiles::open(&mut c);
        c.attach_buffer(&a, 600, 400);
        ask.run(|s| {
            let w = s.workspaces.active_windows()[0].clone();
            let g = s.space.element_geometry(&w).unwrap();
            s.on_pointer_motion(((g.loc.x + 50) as f64, (g.loc.y + 50) as f64).into(), 1);
        });
        c.until(|a| a.pointer_enter.is_some());
        let cursor = c.compositor.create_surface(&qh, ());
        pointer.set_cursor(c.app.pointer_enter.unwrap(), Some(&cursor), 4, 4);
        cursor.frame(&qh, ());
        c.attach_buffer(&cursor, 24, 24);
        assert!(ask.run(|s| matches!(s.cursor_status, CursorImageStatus::Surface(_))), "the app's cursor is shown");
        ask.run(|s| {
            let o = s.space.outputs().next().cloned().unwrap();
            whiterose_wm::render::send_frame_callbacks(&o, s);
        });
        c.until(|a| a.frames_done > 0);
    });
}
