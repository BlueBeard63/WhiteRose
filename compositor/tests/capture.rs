//! Live window previews: wlr-foreign-toplevel-management (window handles) and
//! hyprland-toplevel-export (window pixels), as QuickShell uses them.

mod common;

use common::*;
use wayland_client::protocol::wl_seat::WlSeat;

#[test]
fn foreign_toplevel_handles_track_windows_and_activate_them() {
    let mut h = Headless::new(true);
    h.run(
        |conn| {
            let mut c = Client::connect(&conn);
            let (globals, _) = wayland_client::globals::registry_queue_init::<App>(&conn).unwrap();
            let qh = c.queue.handle();
            let manager: ZwlrForeignToplevelManagerV1 = globals.bind(&qh, 1..=3, ()).expect("foreign toplevel manager");
            let seat: WlSeat = globals.bind(&qh, 1..=7, ()).unwrap();

            let (_s1, t1) = c.open_window();
            t1.set_app_id("org.gnome.Ptyxis".into());
            t1.set_title("term".into());
            c.app.toplevel_configured = false;
            let (s2, t2) = c.open_window();
            t2.set_app_id("firefox".into());
            t2.set_title("Figma".into());
            c.queue.roundtrip(&mut c.app).unwrap();
            let _ = &manager;

            let snapshot = |c: &Client| {
                let mut v: Vec<(String, String, Vec<u32>)> = c
                    .app
                    .handles
                    .iter()
                    .filter(|(_, i)| !i.closed)
                    .map(|(_, i)| (i.app_id.clone(), i.title.clone(), i.states.clone()))
                    .collect();
                v.sort();
                v
            };
            let before = snapshot(&c);

            // Activate the terminal via its handle (what clicking a preview does).
            let term = c.app.handles.iter().find(|(_, i)| i.app_id == "org.gnome.Ptyxis").unwrap().0.clone();
            term.activate(&seat);
            c.queue.roundtrip(&mut c.app).unwrap();
            let after_activate = snapshot(&c);

            // Close firefox: its handle is told.
            t2.destroy();
            s2.destroy();
            c.queue.roundtrip(&mut c.app).unwrap();
            let firefox_closed = c.app.handles.iter().any(|(_, i)| i.app_id == "firefox" && i.closed);
            (before, after_activate, firefox_closed, c.app.handles.iter().all(|(_, i)| i.done))
        },
        |state, (before, after, firefox_closed, all_done)| {
            const ACTIVATED: u32 = 2;
            assert_eq!(before.len(), 2);
            assert_eq!((before[0].0.as_str(), before[0].1.as_str()), ("firefox", "Figma"));
            assert!(before[0].2.contains(&ACTIVATED), "newest window starts activated: {before:?}");
            assert!(!before[1].2.contains(&ACTIVATED));
            assert!(after[1].2.contains(&ACTIVATED), "activate request focused the terminal: {after:?}");
            assert!(!after[0].2.contains(&ACTIVATED));
            assert!(firefox_closed, "closed event sent");
            assert!(all_done, "every handle got a done after its info");
            assert_eq!(state.foreign_toplevels.handle_count(), 1, "closed window's handle dropped");
        },
    );
}

#[test]
fn toplevel_export_handshake_and_buffer_validation() {
    let mut h = Headless::new(true);
    h.run(
        |conn| {
            let mut c = Client::connect(&conn);
            let (globals, _) = wayland_client::globals::registry_queue_init::<App>(&conn).unwrap();
            let qh = c.queue.handle();
            let _m: ZwlrForeignToplevelManagerV1 = globals.bind(&qh, 1..=3, ()).unwrap();
            let export: hyprland_toplevel_export_manager_v1::HyprlandToplevelExportManagerV1 =
                globals.bind(&qh, 2..=2, ()).expect("toplevel export manager");

            let (surface, _t) = c.open_window();
            c.attach_buffer(&surface, 400, 300);
            c.queue.roundtrip(&mut c.app).unwrap();
            let handle = c.app.handles.first().expect("a handle for the window").0.clone();

            // 1. Asking for the window describes a 400x300 Argb8888 buffer.
            let frame = export.capture_toplevel_with_wlr_toplevel_handle(0, &handle, &qh, ());
            c.until(|a| a.export_events.iter().any(|e| e == "buffer_done"));
            let described = c.app.export_events.clone();

            // 2. A wrong-sized buffer is refused with `failed`.
            let small = c.shm_buffer(100, 100);
            frame.copy(&small, 1);
            c.until(|a| a.export_events.iter().any(|e| e == "failed"));
            frame.destroy();

            // 3. A correct buffer is accepted and waits for the renderer.
            c.app.export_events.clear();
            let frame2 = export.capture_toplevel_with_wlr_toplevel_handle(0, &handle, &qh, ());
            c.until(|a| a.export_events.iter().any(|e| e == "buffer_done"));
            let good = c.shm_buffer(400, 300);
            frame2.copy(&good, 1);
            c.queue.roundtrip(&mut c.app).unwrap();
            let after_good = c.app.export_events.clone();

            // 4. An unknown window fails straight away (v1 request by number).
            c.app.export_events.clear();
            let _bad = export.capture_toplevel(0, 9999, &qh, ());
            c.until(|a| !a.export_events.is_empty());
            (described, after_good, c.app.export_events.clone(), frame2)
        },
        |state, (described, after_good, unknown, _frame2)| {
            assert!(
                described.iter().any(|e| e.starts_with("buffer 400x300 stride 1600") && e.contains("Argb8888")),
                "{described:?}"
            );
            assert!(!after_good.iter().any(|e| e == "failed"), "valid buffer refused: {after_good:?}");
            assert_eq!(state.pending_exports.len(), 1, "valid copy waits for the next frame");
            assert_eq!(unknown, vec!["failed".to_string()]);
        },
    );
}
