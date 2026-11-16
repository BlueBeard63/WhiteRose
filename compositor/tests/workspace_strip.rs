//! The bar's workspace strip: an app's icon dragged from one workspace's
//! cell to another moves that window there by its id, from or to the
//! workspace shown or a hidden one, and nothing switches workspace. The
//! windows event says where each window is on its workspace, shown or
//! not, for the strip's previews.

mod common;

use common::{tiles::*, *};
use whiterose_wm::{ipc::Request, keybind::Action};

#[test]
fn a_window_moves_to_any_workspace_by_its_id() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        for _ in 0..2 {
            let (a, _ta) = open(&mut c);
            c.attach_buffer(&a, 600, 400);
        }
        c.queue.roundtrip(&mut c.app).unwrap();
        ask.run(|s| {
            assert_eq!(
                Request::parse(r#"{"cmd":"move_window","id":7,"n":3}"#).map(Request::into_action),
                Ok(Action::MoveWindow(7, 3))
            );
            let w = s.workspaces.active_windows()[0].clone();
            let id = s.window_id(&w);
            let shown = s.place_of(&w).expect("shown");

            // To workspace 3: hidden, and still at workspace 1.
            s.run_action(Action::MoveWindow(id, 3));
            assert_eq!(s.workspaces.workspace_of(&w), Some(2));
            assert_eq!(s.workspaces.active(), 0, "nothing switches");
            assert!(s.space.element_location(&w).is_none(), "hidden");
            let info = |s: &whiterose_wm::WhiteRose| match s.windows_event() {
                whiterose_wm::ipc::Event::Windows { windows } => windows.into_iter().find(|i| i.id == id).unwrap(),
                _ => unreachable!(),
            };
            let hidden = info(s);
            assert_eq!(hidden.geometry, None, "not on screen");
            assert_eq!(hidden.place.map(|p| (p[2], p[3])), Some((shown.size.w, shown.size.h)), "but placed");

            // Hidden to hidden.
            s.run_action(Action::MoveWindow(id, 5));
            assert_eq!(s.workspaces.workspace_of(&w), Some(4));
            assert!(s.space.element_location(&w).is_none());

            // Back to the workspace shown: shown again.
            s.run_action(Action::MoveWindow(id, 1));
            assert_eq!(s.workspaces.workspace_of(&w), Some(0));
            assert!(s.space.element_location(&w).is_some(), "shown");
            assert!(info(s).geometry.is_some());

            // Not a window: nothing happens.
            s.run_action(Action::MoveWindow(9999, 2));
            assert_eq!(s.workspaces.active_windows().len(), 2);
        });
    });
}
