//! Grouped (tabbed) tiles: dropping a window on a tile's top band groups
//! them, the tab strip shows one at a time, and tabs leave the group when
//! they're dragged out, closed, minimised, or the workspace stops tiling.

mod common;

use common::{tiles::*, *};
use serde_json::json;
use whiterose_wm::{keybind::Action, WhiteRose};

/// The whole area as one grouped tile: the strip, and the window below it.
const STRIP: (i32, i32, i32, i32) = (8, 34, 1424, 58);
const UNDER_STRIP: (i32, i32, i32, i32) = (8, 92, 1424, 708);

/// A strip: where, how many tabs, and which is shown.
type StripSeen = ((i32, i32, i32, i32), usize, usize);

fn strips(s: &WhiteRose) -> Vec<StripSeen> {
    s.tiling
        .strips
        .iter()
        .map(|st| ((st.rect.loc.x, st.rect.loc.y, st.rect.size.w, st.rect.size.h), st.tabs.len(), st.shown))
        .collect()
}

/// The middle of the first strip's tab `i`.
fn tab(s: &WhiteRose, i: usize) -> (f64, f64) {
    let p = s.tiling.strips[0].tab_middle(i).expect("a tab");
    (p.x, p.y)
}

/// The first window dragged onto the top band of the second.
fn group_them(s: &mut WhiteRose) {
    let preview = super_drag(s, BTN_LEFT, (362.0, 417.0), (1078.0, 40.0));
    assert_eq!(preview, Some((724, 34, 708, 58)), "the preview is the strip it joins");
}

#[test]
fn dropping_on_the_top_band_groups_the_windows() {
    let mut h = Headless::new(true);
    two_tiles(&mut h, |s| {
        group_them(s);
        // One tile for both: the dropped window shows below the strip, the
        // other is hidden.
        assert_eq!(rects(s), vec![Some(UNDER_STRIP), None]);
        assert_eq!(strips(s), vec![(STRIP, 2, 1)]);
        let ws = all(s);
        assert_eq!(s.tiling.groups.tabs_of(&ws[0]), Some(&[ws[1].clone(), ws[0].clone()][..]));
    });
}

#[test]
fn clicking_a_tab_shows_its_window() {
    let mut h = Headless::new(true);
    two_tiles(&mut h, |s| {
        group_them(s);
        // The first tab (the window it joined).
        let tab = tab(s, 0);
        drag(s, BTN_LEFT, tab, tab);
        let ws = all(s);
        assert_eq!(rects(s), vec![None, Some(UNDER_STRIP)]);
        assert_eq!(strips(s), vec![(STRIP, 2, 0)]);
        assert_eq!(s.focused_window(), Some(ws[1].clone()));
        // Focusing the hidden one (ALT+TAB, the dock) shows it again.
        let id = s.window_id(&ws[0]);
        s.run_action(Action::FocusWindow(id));
        assert_eq!(rects(s), vec![Some(UNDER_STRIP), None]);
    });
}

#[test]
fn dragging_a_tab_out_splits_it_off() {
    let mut h = Headless::new(true);
    set(&mut h.state, "windows.layout", json!("dwindle"));
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let mut surfaces = Vec::new();
        for _ in 0..3 {
            surfaces.push(open(&mut c));
        }
        // Left, right top, right bottom.
        for (s, (w, hh)) in surfaces.iter().zip([(708, 766), (708, 379), (708, 379)]) {
            c.attach_buffer(&s.0, w, hh);
        }
        ask.run(|s| {
            assert_eq!(rects(s)[2], Some((724, 421, 708, 379)));
            // The third onto the first's band.
            super_drag(s, BTN_LEFT, (1078.0, 600.0), (362.0, 40.0));
            assert_eq!(rects(s), vec![None, Some(RIGHT), Some((8, 92, 708, 708))]);
            // The first's tab dragged to the bottom edge of the second: it
            // splits that, and the group (one left) ends.
            let tab = tab(s, 0);
            drag(s, BTN_LEFT, tab, (1078.0, 780.0));
            assert_eq!(rects(s), vec![Some((724, 421, 708, 379)), Some((724, 34, 708, 379)), Some(LEFT)]);
            assert!(strips(s).is_empty());
            assert_eq!(s.tiling.groups.iter().count(), 0);
        });
    });
}

#[test]
fn closing_the_shown_tab_shows_the_next() {
    let mut h = Headless::new(true);
    set(&mut h.state, "windows.layout", json!("dwindle"));
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let (a, ta) = open(&mut c);
        let (b, _tb) = open(&mut c);
        c.attach_buffer(&a, LEFT.2, LEFT.3);
        c.attach_buffer(&b, RIGHT.2, RIGHT.3);
        ask.run(group_them);
        // Read (and ack) what the grouping sent, as a real client would.
        c.queue.roundtrip(&mut c.app).unwrap();
        ta.destroy();
        a.destroy();
        c.queue.roundtrip(&mut c.app).unwrap();
        ask.run(|s| {
            // The other is back, on its own: the whole area, no strip.
            assert_eq!(rects(s), vec![Some(AREA)]);
            assert!(strips(s).is_empty());
            assert_eq!(s.focused_window(), Some(all(s)[0].clone()));
        });
    });
}

#[test]
fn minimising_a_tab_takes_it_out_of_the_group() {
    let mut h = Headless::new(true);
    two_tiles(&mut h, |s| {
        group_them(s);
        let ws = all(s);
        s.minimize(&ws[0]);
        assert_eq!(rects(s), vec![None, Some(AREA)]);
        // Back from the dock, it's a tile of its own.
        s.focus_window(Some(ws[0].clone()));
        assert_eq!(rects(s), vec![Some(RIGHT), Some(LEFT)]);
        assert!(strips(s).is_empty());
    });
}

#[test]
fn hidden_tabs_stay_hidden_across_workspace_switches() {
    let mut h = Headless::new(true);
    two_tiles(&mut h, |s| {
        group_them(s);
        s.switch_workspace(1);
        assert!(strips(s).is_empty(), "no strip on the other workspace");
        s.switch_workspace(0);
        assert_eq!(rects(s), vec![Some(UNDER_STRIP), None]);
        assert_eq!(strips(s), vec![(STRIP, 2, 1)]);
    });
}

#[test]
fn a_workspace_that_stops_tiling_ends_its_groups() {
    let mut h = Headless::new(true);
    two_tiles(&mut h, |s| {
        group_them(s);
        set(s, "windows.layout", json!("floating"));
        // Both float where they first opened.
        assert_eq!(rects(s), vec![Some((48, 74, 0, 0)), Some((80, 106, 0, 0))]);
        assert!(strips(s).is_empty());
        assert_eq!(s.tiling.groups.iter().count(), 0);
    });
}
