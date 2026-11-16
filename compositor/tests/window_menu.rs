//! The window menu: an app asks for it (its title bar right-clicked) or
//! ALT+SPACE opens it for the focused window; the shell is told whose and
//! where. It closes when asked, after closing the window from it, or when
//! the window goes.

mod common;

use common::{tiles::*, *};
use whiterose_wm::{ipc::Event, keybind::Action};

#[test]
fn an_app_asks_for_its_window_menu() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let (a, ta) = open(&mut c);
        c.attach_buffer(&a, 600, 400);
        c.queue.roundtrip(&mut c.app).unwrap();
        ta.show_window_menu(&c.seat, 0, 40, 12);
        c.queue.roundtrip(&mut c.app).unwrap();
        ask.run(|s| {
            let w = s.workspaces.active_windows()[0].clone();
            let at = s.space.element_location(&w).unwrap();
            let Event::WindowMenu { menu: Some(m) } = s.window_menu_event() else { panic!("no menu") };
            assert_eq!(m.id, s.window_id(&w));
            assert_eq!((m.x, m.y), (at.x + 40, at.y + 12), "where it was asked for");
            s.run_action(Action::CloseWindowMenu);
            assert!(s.window_menu.is_none());

            // ALT+SPACE: the focused window's, near its top left.
            s.run_action(Action::WindowMenu);
            assert!(s.window_menu.as_ref().is_some_and(|(m, _)| *m == w));
            // Closed from it: the menu goes too.
            let id = s.window_id(&w);
            s.run_action(Action::CloseWindowById(id));
            assert!(s.window_menu.is_none());
            s.run_action(Action::WindowMenu);
            s.forget_window(&w);
            assert!(s.window_menu.is_none(), "the window went");
        });
    });
}
