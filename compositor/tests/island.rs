//! The idle island tucks into the bar when a window sits under it: the
//! compositor tells the shell whether the area under the island is covered.

mod common;

use common::*;
use whiterose_wm::ipc::Event;

#[test]
fn a_window_under_the_island_covers_it_and_moving_it_away_uncovers_it() {
    let mut h = Headless::new(true);
    assert_eq!(h.state.island_event(), Event::Island { covered: false }, "empty desktop");
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
        assert_eq!(under, Event::Island { covered: true });
        assert_eq!(below, Event::Island { covered: false });
    });
}
