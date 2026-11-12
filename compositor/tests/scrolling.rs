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

mod keys {
    //! The column keys and the existing window keys on a strip, pressed
    //! through the same key path the backends use (evdev + 8).

    use super::*;
    use smithay::{backend::input::KeyState, input::keyboard::Keycode};

    const SHIFT: u32 = 42 + 8;
    const KEY_W: u32 = 17 + 8;
    const R: u32 = 19 + 8;
    const ESC: u32 = 1 + 8;
    const BRACKET_LEFT: u32 = 26 + 8;
    const BRACKET_RIGHT: u32 = 27 + 8;
    const KEY_UP: u32 = 103 + 8;
    const KEY_LEFT: u32 = 105 + 8;
    const KEY_RIGHT: u32 = 106 + 8;
    const KEY_DOWN: u32 = 108 + 8;

    fn press(s: &mut WhiteRose, keys: &[u32]) {
        for k in keys {
            s.on_key(Keycode::new(*k), KeyState::Pressed, 0);
        }
        for k in keys.iter().rev() {
            s.on_key(Keycode::new(*k), KeyState::Released, 0);
        }
    }

    fn focused(s: &WhiteRose) -> Window {
        s.focused_window().unwrap()
    }

    use smithay::desktop::Window;

    #[test]
    fn super_w_cycles_the_width_and_shift_fills_the_screen() {
        let mut h = Headless::new(true);
        strip_settings(&mut h.state);
        h.drive(|conn, ask| {
            let mut c = Client::connect(&conn);
            open_columns(&mut c, 2);
            ask.run(|s| {
                let ws = all(s);
                press(s, &[SUPER, KEY_W]);
                assert_eq!(rect(s, &ws[1]).map(|r| r.2), Some(W * 2 / 3));
                // Still in view: the strip scrolled to keep its right edge on
                // screen.
                assert_eq!(rect(s, &ws[1]).map(|r| r.0 + r.2), Some(W));
                press(s, &[SUPER, KEY_W]);
                assert_eq!(rect(s, &ws[1]).map(|r| r.2), Some(W / 3));
                press(s, &[SUPER, SHIFT, KEY_W]);
                assert_eq!(rect(s, &ws[1]), Some((0, 0, W, H)), "full width, the strip scrolled to it");
                press(s, &[SUPER, SHIFT, KEY_W]);
                assert_eq!(rect(s, &ws[1]).map(|r| r.2), Some(W / 3), "back to its width");
            });
        });
    }

    #[test]
    fn brackets_pull_a_window_into_the_column_and_push_it_out() {
        let mut h = Headless::new(true);
        strip_settings(&mut h.state);
        h.drive(|conn, ask| {
            let mut c = Client::connect(&conn);
            open_columns(&mut c, 3);
            ask.run(|s| {
                let ws = all(s);
                s.focus_window(Some(ws[1].clone()));
                press(s, &[SUPER, BRACKET_LEFT]);
                // The third window joins the second's column, below it.
                assert_eq!(rect(s, &ws[1]).map(|r| (r.0, r.1, r.3)), Some((HALF, 0, H / 2)));
                assert_eq!(rect(s, &ws[2]).map(|r| (r.0, r.1, r.3)), Some((HALF, H / 2, H / 2)));
                // Nothing more to the right.
                press(s, &[SUPER, BRACKET_LEFT]);
                assert_eq!(rect(s, &ws[1]).map(|r| r.3), Some(H / 2));
                press(s, &[SUPER, BRACKET_RIGHT]);
                // The second leaves for a column of its own on the right.
                assert_eq!(rect(s, &ws[2]).map(|r| r.3), Some(H));
                assert_eq!(rect(s, &ws[1]), Some((HALF, 0, HALF, H)), "scrolled to it");
                assert_eq!(rect(s, &ws[2]).map(|r| r.0), Some(0));
            });
        });
    }

    #[test]
    fn arrows_walk_the_strip_and_move_columns() {
        let mut h = Headless::new(true);
        strip_settings(&mut h.state);
        h.drive(|conn, ask| {
            let mut c = Client::connect(&conn);
            open_columns(&mut c, 3);
            ask.run(|s| {
                let ws = all(s);
                // From the third to the first, off screen: the strip follows.
                press(s, &[SUPER, KEY_LEFT]);
                press(s, &[SUPER, KEY_LEFT]);
                assert_eq!(focused(s), ws[0]);
                assert_eq!(rect(s, &ws[0]).map(|r| r.0), Some(0));
                press(s, &[SUPER, KEY_LEFT]);
                assert_eq!(focused(s), ws[0], "the strip's start");
                // SUPER+SHIFT+right takes the whole column along.
                press(s, &[SUPER, SHIFT, KEY_RIGHT]);
                press(s, &[SUPER, SHIFT, KEY_RIGHT]);
                assert_eq!(s.tiling.sets[0].tiling.windows(), vec![ws[1].clone(), ws[2].clone(), ws[0].clone()]);
                assert_eq!(rect(s, &ws[0]), Some((HALF, 0, HALF, H)), "in view at the end");
                // Up and down within a column.
                press(s, &[SUPER, KEY_LEFT]);
                press(s, &[SUPER, BRACKET_LEFT]); // the first joins the third's column
                assert_eq!(focused(s), ws[2]);
                press(s, &[SUPER, KEY_DOWN]);
                assert_eq!(focused(s), ws[0]);
                press(s, &[SUPER, SHIFT, KEY_UP]);
                assert_eq!(rect(s, &ws[0]).map(|r| r.1), Some(0), "moved to the top of its column");
            });
        });
    }

    #[test]
    fn past_the_strips_end_focus_goes_to_the_next_monitor() {
        let mut h = Headless::new(true);
        h.add_output("TEST-2", (W, 0));
        strip_settings(&mut h.state);
        h.drive(|conn, ask| {
            let mut c = Client::connect(&conn);
            open_columns(&mut c, 2);
            // A window on the second monitor (scrolling there too: a strip of
            // its own).
            ask.run(|s| s.on_pointer_motion((W as f64 + 100.0, 100.0).into(), 1));
            open_columns(&mut c, 1);
            ask.run(|s| {
                let ws = all(s);
                assert_eq!(rect(s, &ws[2]), Some((W, 0, HALF, H)));
                s.focus_window(Some(ws[0].clone()));
                press(s, &[SUPER, KEY_RIGHT]);
                assert_eq!(focused(s), ws[1], "the strip first");
                press(s, &[SUPER, KEY_RIGHT]);
                assert_eq!(focused(s), ws[2], "then the next monitor");
            });
        });
    }

    #[test]
    fn resize_mode_changes_width_and_height() {
        let mut h = Headless::new(true);
        strip_settings(&mut h.state);
        h.drive(|conn, ask| {
            let mut c = Client::connect(&conn);
            open_columns(&mut c, 2);
            ask.run(|s| {
                let ws = all(s);
                press(s, &[SUPER, R]);
                press(s, &[KEY_RIGHT]);
                let wider = rect(s, &ws[1]).unwrap().2;
                assert!(wider > HALF, "{wider}");
                assert_eq!(rect(s, &ws[1]).map(|r| r.0 + r.2), Some(W), "kept in view");
                press(s, &[KEY_LEFT]);
                assert_eq!(rect(s, &ws[1]).map(|r| r.2), Some(HALF));
                // A lone window in its column has no height to change.
                press(s, &[KEY_DOWN]);
                assert_eq!(rect(s, &ws[1]).map(|r| r.3), Some(H));
                press(s, &[ESC]);
            });
        });
    }
}
