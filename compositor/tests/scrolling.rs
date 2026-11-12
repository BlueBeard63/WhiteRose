//! Scrolling strips with real clients: new windows open in columns to the
//! right, focus scrolls the strip, the centring setting applies, and a
//! window scrolled off its output never shows or takes the pointer on the
//! monitor next to it.

mod common;

use common::{tiles::*, *};
use serde_json::json;
use whiterose_wm::{layout::Layout, WhiteRose};

/// Half the output's width: a new column, with no gaps.
const HALF: i32 = W / 2;

/// Scrolling everywhere, no gaps (no bar either: the whole output is
/// usable), so columns are exact halves.
fn strip_settings(s: &mut WhiteRose) {
    set(s, "windows.layout", json!("scrolling"));
    set(s, "windows.gaps.inner", json!(0));
    set(s, "windows.gaps.outer", json!(0));
}

/// Opens `n` windows that draw at a column's size, so they have a size for
/// the pointer and the renderer.
fn open_columns(c: &mut Client, n: usize) {
    for _ in 0..n {
        let (surface, _toplevel) = open(c);
        c.attach_buffer(&surface, HALF, H);
    }
    c.queue.roundtrip(&mut c.app).unwrap();
}

#[test]
fn new_windows_open_to_the_right_and_the_strip_follows() {
    let mut h = Headless::new(true);
    strip_settings(&mut h.state);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        open_columns(&mut c, 3);
        ask.run(|s| {
            assert_eq!(s.layout_of(0, &s.space.outputs().next().unwrap().clone()), Layout::Scrolling);
            // The third has focus: the strip scrolled just enough to show
            // it, and the first is off the left edge.
            assert_eq!(rects(s), vec![Some((-HALF, 0, HALF, H)), Some((0, 0, HALF, H)), Some((HALF, 0, HALF, H))]);
            assert!(all(s).iter().all(told_tiled));
        });
    });
}

#[test]
fn focusing_a_window_scrolls_it_into_view() {
    let mut h = Headless::new(true);
    strip_settings(&mut h.state);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        open_columns(&mut c, 3);
        ask.run(|s| {
            let ws = all(s);
            // From the third to the first: they won't fit side by side, so
            // the first would be centred, but the strip stops at its start.
            s.focus_window(Some(ws[0].clone()));
            assert_eq!(rects(s), vec![Some((0, 0, HALF, H)), Some((HALF, 0, HALF, H)), Some((W, 0, HALF, H))]);
            // Already in view: nothing moves.
            s.focus_window(Some(ws[1].clone()));
            assert_eq!(rect(s, &ws[0]), Some((0, 0, HALF, H)));
            s.focus_window(Some(ws[2].clone()));
            assert_eq!(rect(s, &ws[2]), Some((HALF, 0, HALF, H)));
        });
    });
}

#[test]
fn always_centres_the_focused_column() {
    let mut h = Headless::new(true);
    strip_settings(&mut h.state);
    set(&mut h.state, "windows.scroll.center", json!("always"));
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        open_columns(&mut c, 2);
        ask.run(|s| {
            let ws = all(s);
            let centre = (W - HALF) / 2;
            assert_eq!(rect(s, &ws[1]), Some((centre, 0, HALF, H)));
            s.focus_window(Some(ws[0].clone()));
            assert_eq!(rect(s, &ws[0]), Some((centre, 0, HALF, H)), "even the first column");
            // Settings apply live; back to the default, the strip stays
            // against the left edge.
            set(s, "windows.scroll.center", json!("overflow"));
            assert_eq!(rect(s, &ws[0]), Some((0, 0, HALF, H)));
            set(s, "windows.scroll.center", json!("bogus"));
            assert_eq!(s.tiling.settings.scroll_center, whiterose_wm::layout::Center::Overflow);
        });
    });
}

#[test]
fn a_window_scrolled_off_its_monitor_stays_off_the_next_one() {
    let mut h = Headless::new(true);
    let second = h.add_output("TEST-2", (W, 0));
    strip_settings(&mut h.state);
    h.drive(move |conn, ask| {
        let mut c = Client::connect(&conn);
        open_columns(&mut c, 3);
        ask.run(move |s| {
            let first = s.space.outputs().next().unwrap().clone();
            let ws = all(s);
            s.focus_window(Some(ws[0].clone()));
            // The third column is past the first monitor's right edge, where
            // the second monitor is.
            assert_eq!(rect(s, &ws[2]), Some((W, 0, HALF, H)));
            assert_eq!(s.output_of(&ws[2]).as_ref(), Some(&first), "still the first monitor's");
            assert!(!s.shows_on(&ws[2], &second), "not drawn on the second monitor");
            assert!(!s.shows_on(&ws[2], &first), "nor on its own, out of sight");
            assert!(s.shows_on(&ws[1], &first));
            assert!(s.window_under((W as f64 + 10.0, 100.0).into()).is_none(), "nor takes the pointer there");
            assert_eq!(s.window_under((10.0, 100.0).into()).map(|(w, _)| w.clone()), Some(ws[0].clone()));
            // The second monitor has a strip of its own, still empty.
            assert!(s.tiling.sets.iter().filter(|t| t.output == second).all(|t| t.tiling.is_empty()));
        });
    });
}

#[test]
fn a_window_partly_on_screen_takes_the_pointer_only_there() {
    let mut h = Headless::new(true);
    let second = h.add_output("TEST-2", (W, 0));
    strip_settings(&mut h.state);
    h.drive(move |conn, ask| {
        let mut c = Client::connect(&conn);
        open_columns(&mut c, 2);
        ask.run(move |s| {
            let first = s.space.outputs().next().unwrap().clone();
            let ws = all(s);
            // Two thirds wide (by mouse, any width): the second column now
            // runs 240px past the first monitor's edge.
            assert!(s.tiling.sets[0].tiling.resize_edge(
                &ws[0],
                whiterose_wm::layout::Dir::Right,
                W * 2 / 3 - HALF,
                whiterose_wm::layout::Rect::new(0, 0, W, H),
                &s.tiling.settings.gaps.clone(),
            ));
            s.retile();
            assert_eq!(rect(s, &ws[1]), Some((W * 2 / 3, 0, HALF, H)));
            assert!(s.shows_on(&ws[1], &first));
            assert!(!s.shows_on(&ws[1], &second));
            assert_eq!(s.window_under((W as f64 - 10.0, 100.0).into()).map(|(w, _)| w.clone()), Some(ws[1].clone()));
            assert!(s.window_under((W as f64 + 10.0, 100.0).into()).is_none());
        });
    });
}

#[test]
fn turning_scrolling_off_floats_every_column_again() {
    let mut h = Headless::new(true);
    strip_settings(&mut h.state);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        open_columns(&mut c, 3);
        ask.run(|s| {
            set(s, "windows.layout", json!("floating"));
            assert!(s.tiling.sets.is_empty());
            // Back where they floated before they were tiled, off screen
            // ones included.
            assert!(!all(s).iter().any(told_tiled));
            assert!(rects(s).iter().all(|r| r.is_some_and(|r| r.0 >= 0 && r.0 < W)), "{:?}", rects(s));
        });
    });
}
