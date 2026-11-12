//! Scratchpads: the window a pad's key started is claimed, shown under the
//! island above everything, hidden by the key, and follows you across
//! workspaces.
//!
//! The test client runs in this process, so "the process the key started"
//! is this one (`expect_scratchpad` with our pid) instead of a spawned
//! command.

mod common;

use common::{tiles::*, *};
use serde_json::{json, Value};
use smithay::desktop::Window;
use std::time::Duration;
use wayland_client::protocol::wl_surface::WlSurface;
use whiterose_wm::{keybind::Action, WhiteRose};

/// The default 60% x 50% of the usable area (1440 x 782 under the bar),
/// centred, against the bar.
const PAD: (i32, i32, i32, i32) = (288, 26, 864, 391);

fn pads(s: &mut WhiteRose, v: Value) {
    if s.scratchpads_setting("windows.scratchpads", &v) {
        s.apply_shortcuts();
    }
}

/// A pad "term"; it shows and hides at once (no slide) unless a test
/// says otherwise.
fn term(s: &mut WhiteRose, extra: Value) {
    s.scratchpads.slide_length = Duration::ZERO;
    let mut pad = json!({"id": "term", "name": "Terminal", "command": "true", "keys": ["SUPER+GRAVE"]});
    pad.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    pads(s, json!([pad]));
}

/// Every window, shown or hidden, by its app id.
fn by_app(s: &WhiteRose, app: &str) -> Window {
    s.space
        .elements()
        .chain(s.hidden_at.iter().map(|(w, _)| w))
        .find(|w| whiterose_wm::ipc::toplevel_ids(w).0.as_deref() == Some(app))
        .cloned()
        .expect("a window with that app id")
}

fn focused_app(s: &WhiteRose) -> Option<String> {
    s.focused_window().and_then(|w| whiterose_wm::ipc::toplevel_ids(&w).0)
}

fn key(s: &mut WhiteRose) {
    s.run_action(Action::Scratchpad("term".into()));
}

/// Opens "editor" (an ordinary window), then "pad" with the term pad
/// waiting for this process.
fn editor_and_pad(c: &mut Client, ask: &Ask) -> (WlSurface, WlSurface) {
    let (a, _ta) = open_with(c, |t| t.set_app_id("editor".into()));
    c.attach_buffer(&a, 400, 300);
    ask.run(|s| s.expect_scratchpad("term", std::process::id()));
    let (b, _tb) = open_with(c, |t| t.set_app_id("pad".into()));
    c.queue.roundtrip(&mut c.app).unwrap();
    c.attach_buffer(&b, PAD.2, PAD.3);
    (a, b)
}

#[test]
fn the_started_window_shows_under_the_island_and_the_key_hides_it() {
    let mut h = Headless::new(true);
    term(&mut h.state, json!({}));
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let (_a, _b) = editor_and_pad(&mut c, &ask);
        let configure = c.app.toplevel_configures.last().cloned().unwrap();
        assert_eq!((configure.0, configure.1), (PAD.2, PAD.3), "told its size before it draws");
        ask.run(|s| {
            let pad = by_app(s, "pad");
            assert!(s.scratchpads.is_pad(&pad));
            assert_eq!(s.workspaces.workspace_of(&pad), None, "on no workspace");
            assert!(s.workspaces.mru().iter().all(|w| w != &pad), "not in the switchers");
            assert_eq!(rect(s, &pad), Some(PAD));
            assert_eq!(focused_app(s).as_deref(), Some("pad"));
            assert_eq!(s.space.elements().last(), Some(&pad), "on top");

            key(s);
            assert_eq!(rect(s, &pad), None, "hidden");
            assert_eq!(focused_app(s).as_deref(), Some("editor"), "focus goes back");

            key(s);
            assert_eq!(rect(s, &pad), Some(PAD));
            assert_eq!(focused_app(s).as_deref(), Some("pad"));

            // Shown but not focused: the key focuses it.
            let editor = by_app(s, "editor");
            s.focus_window(Some(editor));
            assert_eq!(rect(s, &pad), Some(PAD), "stays shown when it loses focus");
            assert_eq!(s.space.elements().last(), Some(&pad), "still above the focused window");
            key(s);
            assert_eq!(focused_app(s).as_deref(), Some("pad"));
        });
    });
}

#[test]
fn it_follows_you_across_workspaces() {
    let mut h = Headless::new(true);
    term(&mut h.state, json!({}));
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let _ = editor_and_pad(&mut c, &ask);
        ask.run(|s| {
            let pad = by_app(s, "pad");
            s.switch_workspace(1);
            assert_eq!(rect(s, &pad), Some(PAD), "still shown on workspace 2");
            assert_eq!(focused_app(s).as_deref(), Some("pad"), "keeping the keyboard");
            key(s);
            assert_eq!(rect(s, &pad), None);
            key(s);
            assert_eq!(rect(s, &pad), Some(PAD));
            assert_eq!(focused_app(s).as_deref(), Some("pad"));
            s.switch_workspace(0);
            assert_eq!(rect(s, &pad), Some(PAD), "and back on workspace 1");
            assert_eq!(s.workspaces.workspace_of(&pad), None);
            // Keys that would put it on a workspace or in the tiles leave it be.
            s.focus_window(Some(pad.clone()));
            s.run_action(Action::MoveToWorkspace(3));
            s.run_action(Action::ToggleFloating);
            assert_eq!(s.workspaces.workspace_of(&pad), None);
            assert_eq!(rect(s, &pad), Some(PAD));
        });
    });
}

#[test]
fn a_pad_that_hides_on_blur_hides_when_another_window_is_focused() {
    let mut h = Headless::new(true);
    term(&mut h.state, json!({"hideOnBlur": true, "place": "centre", "size": [600, 400]}));
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let _ = editor_and_pad(&mut c, &ask);
        ask.run(|s| {
            let pad = by_app(s, "pad");
            assert_eq!(rect(s, &pad), Some((420, 26 + (782 - 400) / 2, 600, 400)), "centred");
            let editor = by_app(s, "editor");
            s.focus_window(Some(editor));
            assert_eq!(rect(s, &pad), None, "hidden on losing focus");
            assert_eq!(focused_app(s).as_deref(), Some("editor"));
            key(s);
            assert_eq!(focused_app(s).as_deref(), Some("pad"));
        });
    });
}

#[test]
fn only_the_started_process_is_claimed() {
    let mut h = Headless::new(true);
    term(&mut h.state, json!({}));
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        // A pid that isn't this process or an ancestor of it.
        ask.run(|s| s.expect_scratchpad("term", u32::MAX - 1));
        let (a, _ta) = open_with(&mut c, |t| t.set_app_id("other".into()));
        c.attach_buffer(&a, 400, 300);
        ask.run(|s| {
            let other = by_app(s, "other");
            assert!(!s.scratchpads.is_pad(&other));
            assert_eq!(s.workspaces.workspace_of(&other), Some(0), "an ordinary window");
        });
    });
}

#[test]
fn closing_it_turns_the_pad_off_and_removing_the_pad_frees_its_window() {
    let mut h = Headless::new(true);
    term(&mut h.state, json!({}));
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let _ = editor_and_pad(&mut c, &ask);
        // The pad goes from the setting: its window becomes ordinary.
        ask.run(|s| {
            pads(s, json!([]));
            let pad = by_app(s, "pad");
            assert!(!s.scratchpads.is_pad(&pad));
            assert_eq!(s.workspaces.workspace_of(&pad), Some(0));
            assert!(rect(s, &pad).is_some());
        });

        // Back as a pad, then closed: it's forgotten.
        ask.run(|s| {
            term(s, json!({}));
            s.expect_scratchpad("term", std::process::id());
        });
        let (b, tb) = open_with(&mut c, |t| t.set_app_id("pad2".into()));
        c.attach_buffer(&b, PAD.2, PAD.3);
        ask.run(|s| assert!(s.scratchpads.is_pad(&by_app(s, "pad2"))));
        tb.destroy();
        b.destroy();
        c.queue.roundtrip(&mut c.app).unwrap();
        ask.run(|s| {
            assert!(s.scratchpads.focus.is_none());
            assert_eq!(focused_app(s).as_deref(), Some("pad"), "focus moves on");
        });
    });
}

#[test]
fn it_shows_over_a_fullscreen_window() {
    let mut h = Headless::new(true);
    term(&mut h.state, json!({}));
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let (a, _ta) = open_with(&mut c, |t| t.set_app_id("video".into()));
        c.attach_buffer(&a, 400, 300);
        ask.run(|s| {
            let v = by_app(s, "video");
            s.fullscreen(&v);
            s.expect_scratchpad("term", std::process::id());
        });
        let (b, _tb) = open_with(&mut c, |t| t.set_app_id("pad".into()));
        c.attach_buffer(&b, PAD.2, PAD.3);
        ask.run(|s| {
            let v = by_app(s, "video");
            let pad = by_app(s, "pad");
            assert!(s.fullscreen.contains(&v), "the video stays fullscreen");
            assert_eq!(s.space.elements().last(), Some(&pad), "the pad is above it");
            assert_eq!(s.window_under((300.0, 100.0).into()).map(|(w, _)| w.clone()), Some(pad));
        });
    });
}

#[test]
fn it_slides_out_letting_the_pointer_through_and_turns_round_mid_way() {
    let mut h = Headless::new(true);
    term(&mut h.state, json!({}));
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let _ = editor_and_pad(&mut c, &ask);
        ask.run(|s| {
            s.scratchpads.slide_length = Duration::from_millis(60);
            let pad = by_app(s, "pad");
            key(s);
            assert_eq!(rect(s, &pad), Some(PAD), "still drawn while it slides up");
            assert!(s.scratchpads.sliding_out(&pad));
            assert_eq!(focused_app(s).as_deref(), Some("editor"), "the keyboard goes back at once");
            let under = s.window_under((300.0, 100.0).into()).map(|(w, _)| w.clone());
            assert_ne!(under, Some(pad), "clicks go through it");
        });
        std::thread::sleep(Duration::from_millis(150));
        ask.run(|s| {
            let pad = by_app(s, "pad");
            assert_eq!(rect(s, &pad), None, "gone once the slide ends");
            assert_eq!(s.scratchpads.slide_of(&pad), None);

            // Shown, then hidden again straight away: it goes back up from
            // where it got to, so sooner than a whole slide.
            key(s);
            assert_eq!(rect(s, &pad), Some(PAD), "in place at once (it's drawn sliding)");
            assert!(s.scratchpads.slide_of(&pad).is_some_and(|sl| sl.showing));
            key(s);
            assert!(s.scratchpads.sliding_out(&pad));
        });
        std::thread::sleep(Duration::from_millis(150));
        ask.run(|s| {
            let pad = by_app(s, "pad");
            assert_eq!(rect(s, &pad), None);
            assert!(!s.scratchpads.animating());
        });
    });
}
