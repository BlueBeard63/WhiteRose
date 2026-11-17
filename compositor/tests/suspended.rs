//! A window out of sight (another workspace or activity, minimized, a
//! background tab) is told it's suspended, so it can stop drawing and
//! throttle its work; back in sight it isn't.

mod common;

use common::{tiles::*, *};
use whiterose_wm::keybind::Action;

/// xdg_toplevel's `suspended` state.
const SUSPENDED: u32 = 9;

fn suspended(c: &Client) -> bool {
    c.app.toplevel_configures.last().is_some_and(|(_, _, s)| s.contains(&SUSPENDED))
}

#[test]
fn another_workspace_suspends_it_and_coming_back_wakes_it() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        let (a, _ta) = open(&mut c);
        c.attach_buffer(&a, 600, 400);
        c.queue.roundtrip(&mut c.app).unwrap();
        assert!(!suspended(&c));

        ask.run(|s| s.run_action(Action::Workspace(2)));
        c.queue.roundtrip(&mut c.app).unwrap();
        assert!(suspended(&c), "on another workspace it's suspended");

        ask.run(|s| s.run_action(Action::Workspace(1)));
        c.queue.roundtrip(&mut c.app).unwrap();
        assert!(!suspended(&c), "back, it isn't");
    });
}
