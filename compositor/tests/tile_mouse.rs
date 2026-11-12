//! Moving and resizing tiles (and floating windows) with the mouse, driven
//! through the same pointer and key entry points the backends use.

mod common;

use common::{tiles::*, *};

#[test]
fn super_drag_onto_the_middle_of_a_tile_swaps_them() {
    let mut h = Headless::new(true);
    two_tiles(&mut h, |s| {
        let preview = super_drag(s, BTN_LEFT, (362.0, 417.0), (1078.0, 417.0));
        assert_eq!(preview, Some(RIGHT), "the preview covers the tile it swaps with");
        assert_eq!(rects(s), vec![Some(RIGHT), Some(LEFT)]);
        assert_eq!(s.tiling.preview.rect, None, "the preview goes when it's dropped");
        assert_eq!(s.tiling.dragging, None);
    });
}

#[test]
fn dropping_near_an_edge_splits_that_tile() {
    let mut h = Headless::new(true);
    two_tiles(&mut h, |s| {
        // Near the bottom of the right tile: the lower half of it.
        let preview = super_drag(s, BTN_LEFT, (362.0, 417.0), (1078.0, 780.0));
        assert_eq!(preview, Some((724, 417, 708, 383)));
        // The left one left, so the right one's split fills the area: it on
        // top, the dropped one below.
        assert_eq!(rects(s), vec![Some((8, 421, 1424, 379)), Some((8, 34, 1424, 379))]);
    });
}

#[test]
fn dropping_outside_the_tiles_floats_it_there() {
    let mut h = Headless::new(true);
    two_tiles(&mut h, |s| {
        // Onto the bar: outside the tiled area.
        let preview = super_drag(s, BTN_LEFT, (362.0, 417.0), (300.0, 10.0));
        assert_eq!(preview, None, "no preview where it would float");
        let ws = all(s);
        assert!(!s.is_tiled(&ws[0]) && !told_tiled(&ws[0]));
        // Where the drag left it (kept below the bar), at its tile's size.
        assert_eq!(rect(s, &ws[0]), Some((-54, 26, 708, 766)));
        assert_eq!(rect(s, &ws[1]), Some(AREA), "the other takes the space");
        // It stays floating when anything re-tiles.
        s.retile();
        assert!(!s.is_tiled(&ws[0]));
    });
}

#[test]
fn letting_go_between_tiles_puts_it_back() {
    let mut h = Headless::new(true);
    two_tiles(&mut h, |s| {
        // On its own (now empty) place.
        super_drag(s, BTN_LEFT, (362.0, 417.0), (300.0, 400.0));
        assert_eq!(rects(s), vec![Some(LEFT), Some(RIGHT)]);
    });
}

#[test]
fn dragging_the_gap_resizes_both_tiles() {
    let mut h = Headless::new(true);
    two_tiles(&mut h, |s| {
        let focused = s.focused_window();
        drag(s, BTN_LEFT, (720.0, 400.0), (820.0, 400.0));
        assert_eq!(rects(s), vec![Some((8, 34, 808, 766)), Some((824, 34, 608, 766))]);
        assert_eq!(s.focused_window(), focused, "grabbing the gap doesn't change focus");
    });
}

#[test]
fn super_right_drag_resizes_a_tile_from_the_nearest_corner() {
    let mut h = Headless::new(true);
    two_tiles(&mut h, |s| {
        // Lower right of the left tile: its right edge (its bottom is the
        // area's border and stays).
        super_drag(s, BTN_RIGHT, (600.0, 600.0), (700.0, 650.0));
        assert_eq!(rects(s), vec![Some((8, 34, 808, 766)), Some((824, 34, 608, 766))]);
    });
}

#[test]
fn super_drag_moves_and_resizes_floating_windows() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let (a, _t) = open(&mut c);
        c.attach_buffer(&a, 400, 300);
        ask.run(|s| {
            assert_eq!(rects(s), vec![Some((48, 74, 0, 0))]);
            super_drag(s, BTN_LEFT, (100.0, 100.0), (300.0, 200.0));
            assert_eq!(rects(s), vec![Some((248, 174, 0, 0))]);
            // From the lower-right corner: wider and taller.
            super_drag(s, BTN_RIGHT, (600.0, 450.0), (650.0, 490.0));
            assert_eq!(rects(s), vec![Some((248, 174, 450, 340))]);
        });
    });
}

/// An app that won't shrink to its tile (a minimum size, as Ghostty has)
/// draws wider than the tile: it's cut to its tile, so it never covers its
/// neighbour or takes the pointer over it.
#[test]
fn a_window_wider_than_its_tile_is_cut_to_it() {
    let mut h = Headless::new(true);
    set(&mut h.state, "windows.layout", serde_json::json!("dwindle"));
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let (a, _ta) = open(&mut c);
        let (b, _tb) = open(&mut c);
        // The left window ignores its 708px tile and draws 1000px wide.
        c.attach_buffer(&a, 1000, LEFT.3);
        c.attach_buffer(&b, RIGHT.2, RIGHT.3);
        ask.run(|s| {
            let ws = all(s);
            s.focus_window(Some(ws[0].clone())); // on top of the right one
            let tile = s.clip_of(&ws[0]).expect("a tiled window is clipped");
            assert_eq!((tile.loc.x, tile.loc.y, tile.size.w, tile.size.h), LEFT);
            let over_right = (RIGHT.0 as f64 + 50.0, 300.0).into();
            assert_eq!(s.window_under(over_right).map(|(w, _)| w.clone()), Some(ws[1].clone()));
            assert_eq!(s.window_under((50.0, 300.0).into()).map(|(w, _)| w.clone()), Some(ws[0].clone()));
            // Floating again: no clip.
            set(s, "windows.layout", serde_json::json!("floating"));
            assert!(s.clip_of(&ws[0]).is_none());
        });
    });
}
