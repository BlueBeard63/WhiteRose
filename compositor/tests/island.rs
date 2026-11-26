//! The idle island tucks into the bar when a window sits under it, and waits
//! in the top edge over a fullscreen window: the compositor tells the shell
//! which screens are covered, and which have a fullscreen window.

mod common;

use common::*;
use whiterose_wm::ipc::Event;

#[test]
fn a_window_under_the_island_covers_it_and_moving_it_away_uncovers_it() {
    let mut h = Headless::new(true);
    let Event::Island { covered, fullscreen, .. } = h.state.island_event() else { unreachable!() };
    assert!(covered.is_empty() && fullscreen.is_empty(), "empty desktop");
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar(); // reserves the 26 px the island hangs below
        let (surface, _t) = c.open_window();
        c.attach_buffer(&surface, 600, 400);

        // Put it right under the island, then well below it.
        let under = ask.run(|st| {
            let w = st.space.elements().next().unwrap().clone();
            st.space.map_element(w, (W / 2 - 300, BAR as i32), false);
            st.island_event()
        });
        let below = ask.run(|st| {
            let w = st.space.elements().next().unwrap().clone();
            st.space.map_element(w, (W / 2 - 300, 300), false);
            st.island_event()
        });
        let Event::Island { covered, fullscreen, .. } = under else { unreachable!() };
        assert_eq!(covered, vec!["TEST-1".to_string()]);
        assert!(fullscreen.is_empty());
        let Event::Island { covered, fullscreen, .. } = below else { unreachable!() };
        assert!(covered.is_empty() && fullscreen.is_empty());
    });
}

#[test]
fn each_screen_has_its_own_island() {
    let mut h = Headless::new(true);
    h.add_output("TEST-2", (W, 0));
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        let (surface, _t) = c.open_window();
        c.attach_buffer(&surface, 600, 400);
        // Under the second screen's island (its top centre): only that one tucks.
        let second = ask.run(|st| {
            let w = st.space.elements().next().unwrap().clone();
            st.space.map_element(w, (W + W / 2 - 300, 0), false);
            st.island_event()
        });
        let Event::Island { covered, fullscreen, .. } = second else { unreachable!() };
        assert_eq!(covered, vec!["TEST-2".to_string()], "only the screen it tucks under");
        assert!(fullscreen.is_empty());
    });
}

// A view opens on the active screen: the focused window's, so with several
// monitors SUPER+SHIFT+N and the like show on the screen in use.
#[test]
fn the_active_screen_follows_the_focused_window() {
    let mut h = Headless::new(true);
    h.add_output("TEST-2", (W, 0));
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        let (surface, _t) = c.open_window();
        c.attach_buffer(&surface, 600, 400);
        let active = ask.run(|st| {
            let w = st.space.elements().next().unwrap().clone();
            st.space.map_element(w.clone(), (W + 100, 100), false);
            st.focus_window(Some(w));
            let Event::Island { active, .. } = st.island_event() else { unreachable!() };
            active
        });
        assert_eq!(active, "TEST-2", "the focused window's screen is the active one");
    });
}

#[test]
fn a_fullscreen_window_is_reported_for_its_screen_until_it_leaves_fullscreen() {
    let mut h = Headless::new(true);
    h.add_output("TEST-2", (W, 0));
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        let (surface, _t) = c.open_window();
        c.attach_buffer(&surface, 600, 400);
        let full = ask.run(|st| {
            let w = st.space.elements().next().unwrap().clone();
            let o = st.space.outputs().find(|o| o.name() == "TEST-2").cloned();
            st.fullscreen_to(&w, o);
            st.island_event()
        });
        let back = ask.run(|st| {
            let w = st.space.elements().next().unwrap().clone();
            st.unfullscreen(&w);
            st.island_event()
        });
        let Event::Island { fullscreen, .. } = full else { panic!("{full:?}") };
        assert_eq!(fullscreen, vec!["TEST-2".to_string()], "only the screen it fills");
        let Event::Island { fullscreen, .. } = back else { panic!("{back:?}") };
        assert!(fullscreen.is_empty(), "left fullscreen");
    });
}
