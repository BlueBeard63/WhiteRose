//! Activities: each its own workspaces. Switching puts the shown windows
//! aside and brings the other activity's; a window on every activity
//! comes along; a window can be sent to another activity; one deleted,
//! or activities turned off, brings its windows to the current one; an
//! activity's own per-workspace layouts apply only while it's current.

mod common;

use common::{tiles::*, *};
use serde_json::{json, Value};
use smithay::desktop::Window;
use whiterose_wm::{activities::DEFAULT, keybind::Action, WhiteRose};

fn set(s: &mut WhiteRose, key: &str, value: Value) {
    s.daemon_message(&json!({"event": "setting", "key": key, "value": value}));
}

fn shown(s: &WhiteRose, w: &Window) -> bool {
    s.space.element_location(w).is_some()
}

fn by_id(s: &WhiteRose, id: u64) -> Window {
    s.workspaces
        .mru()
        .iter()
        .chain(s.activities.stashed.iter().flat_map(|a| a.workspaces.mru()))
        .find(|w| s.window_id(w) == id)
        .cloned()
        .expect("a window with that id")
}

#[test]
fn each_activity_has_its_own_windows() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let mut surfaces = Vec::new();
        for _ in 0..2 {
            let (a, ta) = open(&mut c);
            c.attach_buffer(&a, 600, 400);
            surfaces.push((a, ta));
        }
        c.queue.roundtrip(&mut c.app).unwrap();
        let (a, b) = ask.run(|s| {
            set(s, "activities.list", json!([{"id": "default", "name": "Main"}, {"id": "home", "name": "Home"}]));
            set(s, "activities.enabled", json!(true));
            let ws = s.workspaces.active_windows().to_vec();
            assert_eq!(ws.len(), 2);
            let (a, b) = (ws[0].clone(), ws[1].clone());

            // To Home: nothing of Main's shows.
            s.run_action(Action::Activity("home".into()));
            assert_eq!(s.activities.current, "home");
            assert!(s.workspaces.mru().is_empty());
            assert!(!shown(s, &a) && !shown(s, &b));
            assert_eq!(s.stashed_activity_of(&a), Some(DEFAULT));
            assert_eq!(s.activities_view()[0].windows, 2);
            (s.window_id(&a), s.window_id(&b))
        });
        // One opened in Home stays in Home.
        let (h1, _th) = open(&mut c);
        c.attach_buffer(&h1, 500, 300);
        c.queue.roundtrip(&mut c.app).unwrap();
        ask.run(move |s| {
            let own = s.workspaces.active_windows().to_vec();
            assert_eq!(own.len(), 1);
            let home_window = own[0].clone();
            s.run_action(Action::Activity(DEFAULT.into()));
            let (a, b) = (by_id(s, a), by_id(s, b));
            assert!(shown(s, &a) && shown(s, &b), "Main's are back");
            assert!(!shown(s, &home_window));
            assert_eq!(s.stashed_activity_of(&home_window), Some("home"));

            // On every activity: it comes along.
            let id = s.window_id(&a);
            s.run_action(Action::WindowActivity(id, None));
            s.run_action(Action::Activity("home".into()));
            assert!(shown(s, &a), "carried across");
            assert!(!shown(s, &b));
            s.run_action(Action::Activity(DEFAULT.into()));
            assert!(shown(s, &a) && shown(s, &b));

            // Sent to Home: gone from here, there when you go.
            let id = s.window_id(&b);
            s.run_action(Action::WindowActivity(id, Some("home".into())));
            assert!(!shown(s, &b));
            assert_eq!(s.stashed_activity_of(&b), Some("home"));
            // Focusing it goes there.
            s.activate_window(&b);
            assert_eq!(s.activities.current, "home");
            assert!(shown(s, &b));

            // Closed while put aside: forgotten there too.
            s.run_action(Action::Activity(DEFAULT.into()));
            s.forget_window(&home_window);
            assert_eq!(s.stashed_activity_of(&home_window), None);

            // Home deleted: its windows come here.
            set(s, "activities.list", json!([{"id": "default", "name": "Main"}]));
            assert!(shown(s, &b));
            assert_eq!(s.stashed_activity_of(&b), None);
        });
        drop(surfaces);
    });
}

#[test]
fn turning_activities_off_brings_every_window_back() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let (a, _ta) = open(&mut c);
        c.attach_buffer(&a, 600, 400);
        c.queue.roundtrip(&mut c.app).unwrap();
        ask.run(|s| {
            set(s, "activities.list", json!([{"id": "default", "name": "Main"}, {"id": "home", "name": "Home"}]));
            set(s, "activities.enabled", json!(true));
            let w = s.workspaces.active_windows()[0].clone();
            let id = s.window_id(&w);
            s.run_action(Action::WindowActivity(id, Some("home".into())));
            s.run_action(Action::Activity("home".into()));
            set(s, "activities.enabled", json!(false));
            assert_eq!(s.activities.current, DEFAULT);
            assert!(shown(s, &w));
            assert!(s.activities.stashed.is_empty());
            // Off: nothing switches.
            s.run_action(Action::Activity("home".into()));
            assert_eq!(s.activities.current, DEFAULT);
        });
    });
}

#[test]
fn an_activitys_own_layouts_apply_only_while_its_current() {
    let mut h = Headless::new(true);
    h.drive(|_conn, ask| {
        ask.run(|s| {
            set(s, "activities.list", json!([{"id": "default", "name": "Main"}, {"id": "home", "name": "Home"}]));
            set(s, "activities.enabled", json!(true));
            set(s, "windows.layout", json!("floating"));
            set(s, "windows.layout.workspaces", json!({"1": "master"}));
            let o = s.space.outputs().next().cloned().unwrap();
            assert_eq!(s.layout_of(0, &o).as_str(), "master");
            s.run_action(Action::Activity("home".into()));
            assert_eq!(s.layout_of(0, &o).as_str(), "master", "the default's until it has its own");
            set(s, "activities.overrides", json!({"home": {"windows.layout.workspaces": {"1": "dwindle"}}}));
            assert_eq!(s.layout_of(0, &o).as_str(), "dwindle");
            s.run_action(Action::Activity(DEFAULT.into()));
            assert_eq!(s.layout_of(0, &o).as_str(), "master");
        });
    });
}

#[test]
fn super_shift_a_picks_the_next_activity_or_a_new_one() {
    let mut h = Headless::new(true);
    h.drive(|_conn, ask| {
        ask.run(|s| {
            // Off: nothing opens.
            s.run_action(Action::ActivityPicker);
            assert!(s.activities.picker.is_none());

            set(s, "activities.list", json!([{"id": "default", "name": "Main"}, {"id": "home", "name": "Home"}]));
            set(s, "activities.enabled", json!(true));
            s.run_action(Action::ActivityPicker);
            assert_eq!(s.activities.picker.map(|p| p.0), Some(1), "the next one chosen");
            s.run_action(Action::ActivityPicker);
            assert_eq!(s.activities.picker.map(|p| p.0), Some(2), "then the new-activity card");
            s.run_action(Action::ActivityPicker);
            assert_eq!(s.activities.picker.map(|p| p.0), Some(0), "round again");
            s.activity_picker_step(1);
            s.commit_activity_picker();
            assert!(s.activities.picker.is_none());
            assert_eq!(s.activities.current, "home");

            // The new-activity card (from Home the next is Main; back one
            // is the last card): one more, and you're on it.
            s.run_action(Action::ActivityPicker);
            assert_eq!(s.activities.picker.map(|p| p.0), Some(0));
            s.activity_picker_step(-1);
            assert_eq!(s.activities.picker.map(|p| p.0), Some(2));
            s.commit_activity_picker();
            assert_eq!(s.activities.order.len(), 3);
            assert_eq!(s.activities.current, s.activities.order[2]);
            assert_eq!(s.activities.entries[2].name, "Activity 2");

            // A click on a card from the shell.
            s.run_action(Action::ActivityPicker);
            s.run_action(Action::Activity(DEFAULT.into()));
            assert!(s.activities.picker.is_none());
            assert_eq!(s.activities.current, DEFAULT);
        });
    });
}

// The picker (SUPER+SHIFT+A): its pencil renames an activity (the
// compositor edits the name), its trash deletes one; the default stays.
#[test]
fn the_picker_renames_and_deletes_an_activity() {
    let mut h = Headless::new(true);
    h.drive(|_conn, ask| {
        ask.run(|s| {
            set(s, "activities.list", json!([{"id": "default", "name": "Main"}, {"id": "home", "name": "Home"}]));
            set(s, "activities.enabled", json!(true));
            s.run_action(Action::ActivityPicker);

            // Pencil: open the name field with the current name, type a new one.
            s.start_activity_rename("home");
            assert_eq!(s.activities.rename.as_ref().map(|r| r.buffer.as_str()), Some("Home"));
            for _ in 0..4 {
                s.activity_rename_backspace();
            }
            for c in "Work".chars() {
                s.activity_rename_char(c);
            }
            s.commit_activity_rename();
            assert!(s.activities.rename.is_none(), "the field closes on commit");
            assert_eq!(s.activities.entries.iter().find(|e| e.id == "home").map(|e| e.name.as_str()), Some("Work"));

            // A blank name is ignored: the old one stays.
            s.start_activity_rename("home");
            for _ in 0..20 {
                s.activity_rename_backspace();
            }
            s.commit_activity_rename();
            assert_eq!(s.activities.entries.iter().find(|e| e.id == "home").map(|e| e.name.as_str()), Some("Work"));

            // Esc drops the typing.
            s.start_activity_rename("home");
            s.activity_rename_char('Z');
            s.cancel_activity_rename();
            assert_eq!(s.activities.entries.iter().find(|e| e.id == "home").map(|e| e.name.as_str()), Some("Work"));

            // Trash: the activity goes; the default can't be deleted.
            s.delete_activity("home");
            assert_eq!(s.activities.order, vec![DEFAULT.to_string()]);
            s.delete_activity(DEFAULT);
            assert_eq!(s.activities.order, vec![DEFAULT.to_string()], "the default activity stays");
        });
    });
}

#[test]
fn a_rule_opens_a_window_on_every_activity_or_another_one() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        ask.run(|s| {
            set(s, "activities.list", json!([{"id": "default", "name": "Main"}, {"id": "home", "name": "Home"}]));
            set(s, "activities.enabled", json!(true));
            set(
                s,
                "windows.rules",
                json!([{"app": "^chat$", "activity": "every"}, {"app": "^music$", "activity": "home"}]),
            );
        });
        let (a, _ta) = open_with(&mut c, |t| t.set_app_id("chat".into()));
        c.attach_buffer(&a, 400, 300);
        let (b, _tb) = open_with(&mut c, |t| t.set_app_id("music".into()));
        c.attach_buffer(&b, 400, 300);
        c.queue.roundtrip(&mut c.app).unwrap();
        ask.run(|s| {
            let chat = s.workspaces.active_windows().iter().find(|w| s.window_id(w) == 1).cloned().expect("chat here");
            assert!(s.activities.everywhere.contains(&chat));
            let music =
                s.activities.stashed.iter().find(|a| a.id == "home").expect("Home has one").workspaces.mru()[0].clone();
            assert!(!shown(s, &music), "it opened quietly on Home");
            assert_eq!(s.activities.current, DEFAULT);
            s.run_action(Action::Activity("home".into()));
            assert!(shown(s, &music) && shown(s, &chat));
        });
    });
}
