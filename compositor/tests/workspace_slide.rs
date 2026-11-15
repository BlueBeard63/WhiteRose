//! Switching workspace slides the drawing, not the windows: they're where
//! they'll be at once, and the setting turns the slide off.

mod common;

use common::{tiles::*, *};
use serde_json::json;
use std::time::Instant;

#[test]
fn switching_slides_and_the_setting_turns_it_off() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let (a, _ta) = open(&mut c);
        c.attach_buffer(&a, 400, 300);
        c.queue.roundtrip(&mut c.app).unwrap();
        ask.run(|s| {
            let w = s.space.elements().next().cloned().expect("a window");
            let at = s.space.element_location(&w);

            s.switch_workspace(2);
            let now = Instant::now();
            let slide = *s.workspace_sliding(now).expect("sliding");
            assert_eq!((slide.from, slide.to), (0, 2));
            assert!(s.animating(), "frames keep coming");
            // Only drawn across: the window is already off the space, the
            // pointer works on workspace 3.
            assert!(s.space.element_location(&w).is_none());
            assert_eq!(s.hidden_at.iter().find(|(h, _)| *h == w).map(|(_, l)| *l), at);
            let dx = s.workspace_shift(Some(0), 1440, now);
            assert!((-1440..=0).contains(&dx), "{dx}: going out to the left");

            // Back before it's done: turned round, not started again.
            s.switch_workspace(0);
            let back = *s.workspace_sliding(Instant::now()).expect("still sliding");
            assert_eq!((back.from, back.to), (0, 2), "the same slide, going back");
            assert_eq!(back.along.target(), 0.0);
            assert!(s.space.element_location(&w).is_some(), "the window's back in place");

            // Off: no slide, and one going stops.
            s.animation_setting("animations.workspaceSwitch", &json!(false));
            assert!(s.workspace_sliding(Instant::now()).is_none());
            s.switch_workspace(1);
            assert!(s.workspace_sliding(Instant::now()).is_none());
            assert!(!s.animating());
            assert_eq!(s.workspace_shift(Some(0), 1440, Instant::now()), 0);
        });
    });
}
