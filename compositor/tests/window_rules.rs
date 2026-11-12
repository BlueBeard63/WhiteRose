//! Window rules: a window that matches opens where and how its rules say,
//! from its first configure.

mod common;

use common::{tiles::*, *};
use serde_json::{json, Value};
use smithay::{desktop::Window, output::Output};
use whiterose_wm::{outputs, WhiteRose};

/// Where an unruled window opens on its own: the cascade, below the bar.
const CASCADE: (i32, i32) = (48, 74);

fn rules(s: &mut WhiteRose, v: Value) {
    s.rules_setting("windows.rules", &v);
}

/// The last configure the client was sent, once the compositor has
/// answered everything sent so far (the rules apply on the first commit).
fn last_configure(c: &mut Client) -> (i32, i32, Vec<u32>) {
    c.queue.roundtrip(&mut c.app).unwrap();
    c.app.toplevel_configures.last().cloned().unwrap()
}

/// The window with app id `app`.
fn by_app(s: &WhiteRose, app: &str) -> Window {
    all(s)
        .into_iter()
        .find(|w| whiterose_wm::ipc::toplevel_ids(w).0.as_deref() == Some(app))
        .expect("a window with that app id")
}

fn workspace_of(s: &WhiteRose, app: &str) -> Option<usize> {
    s.workspaces.workspace_of(&by_app(s, app))
}

fn focused_app(s: &WhiteRose) -> Option<String> {
    s.focused_window().and_then(|w| whiterose_wm::ipc::toplevel_ids(&w).0)
}

#[test]
fn a_rule_opens_a_window_quietly_on_its_workspace() {
    let mut h = Headless::new(true);
    rules(&mut h.state, json!([{"app": "^chat$", "workspace": 3}]));
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let (a, _ta) = open_with(&mut c, |t| t.set_app_id("editor".into()));
        c.attach_buffer(&a, 400, 300);
        let (b, _tb) = open_with(&mut c, |t| t.set_app_id("chat".into()));
        let configure = last_configure(&mut c);
        c.attach_buffer(&b, 400, 300);
        ask.run(move |s| {
            assert_eq!(workspace_of(s, "chat"), Some(2));
            assert_eq!(workspace_of(s, "editor"), Some(0));
            assert_eq!(s.workspaces.active(), 0, "its workspace doesn't show");
            assert_eq!(rect(s, &by_app(s, "chat")), None, "hidden");
            assert_eq!(focused_app(s).as_deref(), Some("editor"), "focus stays");
            assert!(!configure.2.contains(&ACTIVATED), "not told it's focused: {:?}", configure.2);

            s.switch_workspace(2);
            assert_eq!(
                rect(s, &by_app(s, "chat")).map(|r| (r.0, r.1)),
                Some((CASCADE.0 + 32, CASCADE.1 + 32)),
                "there when you go there (the second step of the cascade)"
            );
            assert_eq!(focused_app(s).as_deref(), Some("chat"));
        });
    });
}

#[test]
fn a_rule_floats_sizes_and_centres_a_window_that_would_tile() {
    let mut h = Headless::new(true);
    set(&mut h.state, "windows.layout", json!("dwindle"));
    rules(&mut h.state, json!([{"app": "calc", "float": true, "size": [400, "50%"], "position": "centre"}]));
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let (a, _ta) = open_with(&mut c, |t| t.set_app_id("calc".into()));
        let (w, hh, _) = last_configure(&mut c);
        assert_eq!((w, hh), (400, (H - 26) / 2), "the first configure has the rule's size");
        c.attach_buffer(&a, 400, (H - 26) / 2);
        ask.run(|s| {
            let calc = by_app(s, "calc");
            assert!(!s.is_tiled(&calc));
            assert_eq!(rect(s, &calc), Some(((W - 400) / 2, 26 + (H - 26) / 4, 400, (H - 26) / 2)));
        });
    });
}

#[test]
fn a_tiled_window_ignores_size_and_position() {
    let mut h = Headless::new(true);
    set(&mut h.state, "windows.layout", json!("dwindle"));
    rules(&mut h.state, json!([{"app": "term", "size": [300, 200], "position": [0, 0]}]));
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let (a, _ta) = open_with(&mut c, |t| t.set_app_id("term".into()));
        c.attach_buffer(&a, AREA.2, AREA.3);
        ask.run(|s| {
            assert!(s.is_tiled(&by_app(s, "term")));
            assert_eq!(rects(s), vec![Some(AREA)]);
        });
    });
}

#[test]
fn rules_match_the_title_and_later_ones_win() {
    let mut h = Headless::new(true);
    rules(
        &mut h.state,
        json!([
            {"app": "^files$", "position": [100, 100]},
            {"title": "^Copying", "position": [500, 300]},
            {"app": "nothing", "position": [0, 0]},
        ]),
    );
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let (a, _ta) = open_with(&mut c, |t| t.set_app_id("files".into()));
        c.attach_buffer(&a, 400, 300);
        let (b, _tb) = open_with(&mut c, |t| {
            t.set_app_id("files".into());
            t.set_title("Copying 3 files".into());
        });
        c.attach_buffer(&b, 300, 100);
        let (o, _to) = open_with(&mut c, |t| t.set_app_id("other".into()));
        c.attach_buffer(&o, 300, 100);
        ask.run(|s| {
            let at = |w: &Window| s.space.element_location(w).map(|p| (p.x, p.y));
            let [a, b, o] = all(s).try_into().unwrap();
            assert_eq!(at(&a), Some((100, 126)), "from the usable area's top left");
            assert_eq!(at(&b), Some((500, 326)));
            assert_eq!(at(&o).map(|p| p.1), Some(CASCADE.1 + 64), "no rule: the cascade");
        });
    });
}

#[test]
fn a_rule_opens_a_window_on_its_monitor() {
    let mut h = Headless::new(true);
    let second: Output = h.add_output("T-2", (W, 0));
    outputs::set_info(&second, outputs::OutputInfo { id: "DEL-2".into(), ..Default::default() });
    rules(
        &mut h.state,
        json!([{"app": "music", "monitor": "DEL-2", "maximized": true}, {"app": "gone", "monitor": "X"}]),
    );
    h.drive(move |conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let (a, _ta) = open_with(&mut c, |t| t.set_app_id("music".into()));
        let (w, hh, states) = last_configure(&mut c);
        assert!(states.contains(&MAXIMIZED), "maximized from the first configure: {states:?}");
        assert_eq!((w, hh), (W, H), "the second monitor has no bar");
        c.attach_buffer(&a, W, H);
        let (b, _tb) = open_with(&mut c, |t| t.set_app_id("gone".into()));
        c.attach_buffer(&b, 300, 100);
        ask.run(move |s| {
            assert_eq!(s.output_of(&by_app(s, "music")).as_ref(), Some(&second));
            assert_eq!(s.space.element_location(&by_app(s, "music")).map(|p| (p.x, p.y)), Some((W, 0)));
            assert_ne!(s.output_of(&by_app(s, "gone")).as_ref(), Some(&second), "an unknown monitor is ignored");
        });
    });
}

#[test]
fn a_fullscreen_window_stays_when_a_rule_opens_one_elsewhere() {
    let mut h = Headless::new(true);
    rules(&mut h.state, json!([{"app": "mail", "workspace": 2, "fullscreen": true}]));
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let (a, ta) = open_with(&mut c, |t| t.set_app_id("video".into()));
        c.attach_buffer(&a, 400, 300);
        let seen = c.app.toplevel_configures.len();
        ta.set_fullscreen(None);
        c.next_toplevel_configure(seen);
        let (b, _tb) = open_with(&mut c, |t| t.set_app_id("mail".into()));
        let (_, _, states) = last_configure(&mut c);
        assert!(states.contains(&FULLSCREEN), "fullscreen on its own workspace: {states:?}");
        c.attach_buffer(&b, W, H);
        let (o, _to) = open_with(&mut c, |t| t.set_app_id("other".into()));
        c.attach_buffer(&o, 300, 100);
        ask.run(|s| {
            let video = by_app(s, "video");
            assert!(s.fullscreen.contains(&by_app(s, "mail")));
            assert!(!s.fullscreen.contains(&video), "a window opening here still leaves fullscreen");
            assert_eq!(workspace_of(s, "mail"), Some(1));
        });
    });
}
