//! The overview: SUPER+W spreads the current workspace's windows (none
//! overlapping) under a row of every workspace; the pointer goes to it,
//! not the apps. A click on a window goes to it, on a workspace goes
//! there, and a window dragged onto a workspace moves there. Three fingers
//! up open it following the fingers, down close it; Esc closes it; with
//! `overview.style` off it doesn't open.

mod common;

use common::{tiles::*, *};
use serde_json::json;
use smithay::{
    backend::input::ButtonState,
    utils::{Logical, Point},
};
use whiterose_wm::{keybind::Action, overview::Rect, WhiteRose};

const LEFT: u32 = 0x110;

fn middle(s: &WhiteRose, r: &Rect) -> Point<f64, Logical> {
    let o = s.space.outputs().next().unwrap();
    let at = s.space.output_geometry(o).unwrap().loc.to_f64();
    at + Point::from((r.x + r.w / 2.0, r.y + r.h / 2.0))
}

fn click(s: &mut WhiteRose, at: Point<f64, Logical>) {
    s.on_pointer_motion(at, 1);
    s.on_pointer_button(LEFT, ButtonState::Pressed, 2);
    s.on_pointer_button(LEFT, ButtonState::Released, 3);
}

fn two_windows(c: &mut Client) {
    for _ in 0..2 {
        let (a, _ta) = open(c);
        c.attach_buffer(&a, 600, 400);
    }
    c.queue.roundtrip(&mut c.app).unwrap();
}

#[test]
fn super_w_spreads_the_windows_and_a_click_goes_to_one() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        two_windows(&mut c);
        ask.run(|s| {
            s.run_action(Action::Overview(None));
            assert!(s.overview_active());
            let o = s.space.outputs().next().cloned().unwrap();
            let scene = s.overview_scene(&o).expect("a scene");
            assert_eq!(scene.windows.len(), 2);
            let (a, b) = (scene.windows[0].2, scene.windows[1].2);
            assert!(a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y, "apart");
            assert_eq!(scene.cells.len(), 9);
            let view = s.overview_view().expect("the shell is told");
            assert_eq!(view.outputs[0].windows.len(), 2);

            // The pointer outlines a window, and the app under it doesn't get it.
            let first = scene.windows[0].0.clone();
            s.on_pointer_motion(middle(s, &a), 1);
            assert_eq!(s.overview.hovered.as_ref(), Some(&first));
            assert!(s.seat.get_pointer().unwrap().current_focus().is_none());

            click(s, middle(s, &a));
            assert_eq!(s.overview.open.target(), 0.0, "closing");
            assert_eq!(s.focused_window().as_ref(), Some(&first));

            // Esc closes it too.
            s.run_action(Action::Overview(Some(true)));
            s.run_action(Action::Overview(Some(false)));
            assert_eq!(s.overview.open.target(), 0.0);
        });
    });
}

#[test]
fn workspaces_in_the_row_take_clicks_and_dropped_windows() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        two_windows(&mut c);
        ask.run(|s| {
            s.run_action(Action::Overview(None));
            let o = s.space.outputs().next().cloned().unwrap();
            let scene = s.overview_scene(&o).unwrap();
            let (w, _, r) = scene.windows[0].clone();
            // Dragged onto workspace 4: it moves there, the overview stays.
            s.on_pointer_motion(middle(s, &r), 1);
            s.on_pointer_button(LEFT, ButtonState::Pressed, 2);
            s.on_pointer_motion(middle(s, &r) + Point::from((30.0, 0.0)), 3);
            assert!(s.overview.dragging.is_some());
            s.on_pointer_motion(middle(s, &scene.cells[3]), 4);
            s.on_pointer_button(LEFT, ButtonState::Released, 5);
            assert_eq!(s.workspaces.workspace_of(&w), Some(3));
            assert!(s.overview_active());
            assert_eq!(s.overview_scene(&o).unwrap().windows.len(), 1);
            // A click on workspace 4 goes there; the overview stays.
            click(s, middle(s, &scene.cells[3]));
            assert_eq!(s.workspaces.active(), 3);
            assert!(s.overview_active());
            assert_eq!(s.overview_scene(&o).unwrap().windows.len(), 1);
            // A click on nothing closes it.
            click(s, Point::from((5.0, 800.0)));
            assert_eq!(s.overview.open.target(), 0.0);
        });
    });
}

#[test]
fn three_fingers_open_and_close_it_and_the_style_can_turn_it_off() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        ask.run(|s| {
            s.swipe_begin(3, 0);
            s.swipe_update(1, Point::from((0.0, -20.0)));
            s.swipe_update(2, Point::from((0.0, -105.0)));
            assert!(s.overview.open.driven());
            let half = s.overview_shown();
            assert!((half - 0.5).abs() < 1e-6, "{half}: following the fingers");
            s.swipe_end(3, false);
            assert_eq!(s.overview.open.target(), 1.0, "half way opens it");
            // Down a little and back: stays open.
            s.overview.open.jump(1.0);
            s.swipe_begin(3, 10);
            s.swipe_update(11, Point::from((0.0, 30.0)));
            std::thread::sleep(std::time::Duration::from_millis(100));
            s.swipe_end(12, false);
            assert_eq!(s.overview.open.target(), 1.0);
            // Down all the way closes it.
            s.swipe_begin(3, 20);
            s.swipe_update(21, Point::from((0.0, 300.0)));
            s.swipe_end(22, false);
            assert_eq!(s.overview.open.target(), 0.0);

            s.overview_setting("overview.style", &json!("off"));
            s.run_action(Action::Overview(None));
            assert!(!s.overview_active());
            s.swipe_begin(3, 30);
            s.swipe_update(31, Point::from((0.0, -200.0)));
            assert!(!s.overview_active(), "the gesture is the app's then");
            s.swipe_end(32, false);
        });
    });
}
