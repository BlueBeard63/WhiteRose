//! Touchpad gestures, driven as the backends drive them (swipe begin,
//! updates, end): three-finger swipes switch workspace following the
//! fingers, a bound gesture runs its shortcut when the fingers lift,
//! three fingers drag a window when that's on, and what isn't bound
//! stays the app's.

mod common;

use common::{tiles::*, *};
use serde_json::json;
use smithay::utils::{Logical, Point};
use std::time::Instant;
use whiterose_wm::{gestures::Live, WhiteRose};

fn swipe(s: &mut WhiteRose, fingers: u32, steps: &[(f64, f64)]) {
    s.swipe_begin(fingers, 0);
    for (i, (dx, dy)) in steps.iter().enumerate() {
        s.swipe_update(i as u32 + 1, Point::<f64, Logical>::from((*dx, *dy)));
    }
}

#[test]
fn three_fingers_switch_workspace_and_spring() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        ask.run(|s| {
            // Left 200 of the 300 a workspace takes: two thirds of the way
            // to workspace 2, drawn so before the fingers lift.
            swipe(s, 3, &[(-20.0, 0.0), (-80.0, 0.0), (-100.0, 0.0)]);
            assert!(matches!(s.gesture, Live::Workspace { .. }));
            let slide = *s.workspace_sliding(Instant::now()).expect("following the fingers");
            assert_eq!((slide.from, slide.to), (0, 1));
            assert!((slide.along.get(Instant::now()) - 2.0 / 3.0).abs() < 1e-6);
            assert_eq!(s.workspaces.active(), 0, "not switched until they lift");
            s.swipe_end(10, false);
            assert_eq!(s.workspaces.active(), 1);
            let slide = *s.workspace_sliding(Instant::now()).expect("springing on");
            assert_eq!(slide.along.target(), 1.0);
            assert!(s.animating());

            // A little way and back the other way: stays, springing back.
            swipe(s, 3, &[(20.0, 0.0), (30.0, 0.0)]);
            let slide = *s.workspace_sliding(Instant::now()).unwrap();
            assert_eq!((slide.from, slide.to), (1, 0), "right brings in the one before");
            std::thread::sleep(std::time::Duration::from_millis(100)); // no flick
            s.swipe_end(20, false);
            assert_eq!(s.workspaces.active(), 1);
            assert_eq!(s.workspace_sliding(Instant::now()).unwrap().along.target(), 0.0);

            // Cancelled half way: back.
            swipe(s, 3, &[(-200.0, 0.0)]);
            s.swipe_end(30, true);
            assert_eq!(s.workspaces.active(), 1);

            // Nothing before the first workspace.
            s.switch_workspace(0);
            swipe(s, 3, &[(50.0, 0.0), (100.0, 0.0)]);
            assert!(matches!(s.gesture, Live::Workspace { .. }));
            s.swipe_end(40, false);
            assert_eq!(s.workspaces.active(), 0);
        });
    });
}

#[test]
fn unbound_gestures_stay_the_apps() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        ask.run(|s| {
            swipe(s, 4, &[(-200.0, 0.0)]);
            assert!(matches!(s.gesture, Live::Passing), "four fingers go straight to the app");
            s.swipe_end(5, false);
            s.pinch_begin(2, 0);
            assert!(matches!(s.gesture, Live::Passing), "so does pinch to zoom");
            s.pinch_end(1, false);
            // Three fingers pinching out: held until it's clear, then handed on.
            s.pinch_begin(3, 0);
            s.pinch_update(1, Point::from((0.0, 0.0)), 1.05, 0.0);
            assert!(matches!(s.gesture, Live::Pinch { .. }));
            s.pinch_update(2, Point::from((0.0, 0.0)), 1.2, 0.0);
            assert!(matches!(s.gesture, Live::Passing));
            s.pinch_end(3, false);
            assert_eq!(s.workspaces.active(), 0);
        });
    });
}

#[test]
fn a_bound_gesture_runs_its_shortcut_when_the_fingers_lift() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let (a, _ta) = open(&mut c);
        c.attach_buffer(&a, 400, 300);
        c.queue.roundtrip(&mut c.app).unwrap();
        ask.run(|s| {
            s.gestures_setting("gestures.bindings", &json!({"3-swipe-up": "fullscreen", "4-pinch-in": "nope"}));
            let w = s.focused_window().expect("focused");
            swipe(s, 3, &[(0.0, -30.0), (0.0, -30.0)]);
            assert!(!s.fullscreen.contains(&w), "not yet");
            s.swipe_end(5, false);
            assert!(s.fullscreen.contains(&w));
            // Not a shortcut that exists: the app's.
            s.pinch_begin(4, 0);
            s.pinch_update(1, Point::from((0.0, 0.0)), 0.7, 0.0);
            assert!(matches!(s.gesture, Live::Passing));
            s.pinch_end(2, false);
            // The setting replaced the defaults: left or right is the app's now.
            swipe(s, 3, &[(-200.0, 0.0)]);
            assert!(matches!(s.gesture, Live::Passing));
            s.swipe_end(6, false);
        });
    });
}

#[test]
fn three_fingers_drag_a_window_when_that_is_on() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let (a, _ta) = open(&mut c);
        c.attach_buffer(&a, 400, 300);
        c.queue.roundtrip(&mut c.app).unwrap();
        ask.run(|s| {
            s.gestures_setting("gestures.dragWindow", &json!(true));
            let w = s.space.elements().next().cloned().unwrap();
            let at = s.space.element_location(&w).unwrap();
            let middle = (at + Point::from((200, 150))).to_f64();
            s.on_pointer_motion(middle, 0);
            swipe(s, 3, &[(30.0, 10.0), (20.0, 10.0)]);
            assert!(matches!(s.gesture, Live::Drag));
            assert_eq!(s.space.element_location(&w).unwrap(), at + Point::from((50, 20)));
            s.swipe_end(5, false);
            assert!(!s.seat.get_pointer().unwrap().is_grabbed(), "let go");
            // Swipes take four fingers now.
            swipe(s, 4, &[(-200.0, 0.0)]);
            assert!(matches!(s.gesture, Live::Workspace { .. }));
            s.swipe_end(6, false);
            assert_eq!(s.workspaces.active(), 1);
            // Three fingers on no window: the app's (nothing to drag).
            s.on_pointer_motion(Point::from((1400.0, 850.0)), 7);
            swipe(s, 3, &[(-200.0, 0.0)]);
            assert!(matches!(s.gesture, Live::Passing));
            s.swipe_end(8, false);
        });
    });
}
