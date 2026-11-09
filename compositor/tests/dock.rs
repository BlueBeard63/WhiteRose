//! What the dock relies on: the window list over IPC, and focusing by app
//! (cycling) or by window id (switching workspace), driven by real clients.

mod common;

use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    time::Duration,
};

use common::*;
use serde_json::Value;
use whiterose_wm::keybind::Action;

/// Reads IPC events until one matches, returning it.
fn wait_for(reader: &mut BufReader<UnixStream>, pred: impl Fn(&Value) -> bool) -> Value {
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).expect("ipc event within timeout");
        let v: Value = serde_json::from_str(&line).unwrap();
        if pred(&v) {
            return v;
        }
    }
}

fn windows(v: &Value) -> Vec<(String, String, u64, bool)> {
    v["windows"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| {
            (
                w["app_id"].as_str().unwrap_or("").to_string(),
                w["title"].as_str().unwrap_or("").to_string(),
                w["workspace"].as_u64().unwrap(),
                w["focused"].as_bool().unwrap(),
            )
        })
        .collect()
}

#[test]
fn window_list_and_focus_by_app_and_id() {
    let mut h = Headless::new(true);
    let ipc_path = h.state.ipc.as_ref().unwrap().path.clone();

    h.run(
        move |conn| {
            let stream = UnixStream::connect(&ipc_path).unwrap();
            stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
            let mut writer = stream.try_clone().unwrap();
            let mut reader = BufReader::new(stream);

            let mut c = Client::connect(&conn);
            let open = |c: &mut Client, app: &str, title: &str| {
                let (_s, t) = c.open_window();
                t.set_app_id(app.into());
                t.set_title(title.into());
                c.app.toplevel_configured = false;
                c.queue.roundtrip(&mut c.app).unwrap();
                t
            };
            let _t1 = open(&mut c, "org.gnome.Ptyxis", "term one");
            let _b = open(&mut c, "firefox", "WhiteRose — Figma");
            let _t2 = open(&mut c, "org.gnome.Ptyxis", "term two");

            // 1. The list arrives with app ids, titles, workspace and focus, MRU first.
            let ev = wait_for(&mut reader, |v| {
                v["event"] == "windows"
                    && v["windows"].as_array().is_some_and(|a| a.len() == 3 && a.iter().all(|w| w["title"].is_string()))
            });
            let list = windows(&ev);
            assert_eq!(list[0], ("org.gnome.Ptyxis".into(), "term two".into(), 1, true));
            assert_eq!(list[1].0, "firefox");
            assert_eq!(list[2].1, "term one");
            assert!(ev["windows"].as_array().unwrap().iter().all(|w| w["id"].as_u64().unwrap() > 0));
            let firefox_id = ev["windows"][1]["id"].as_u64().unwrap();

            // 2. Clicking the terminal icon while a terminal is focused cycles to the other one.
            writer.write_all(b"{\"cmd\":\"focus_app\",\"app_id\":\"org.gnome.Ptyxis\"}\n").unwrap();
            let ev = wait_for(&mut reader, |v| v["event"] == "windows" && v["windows"][0]["title"] == "term one");
            assert!(windows(&ev)[0].3, "cycled window is focused");

            // 3. Focus a window by id.
            writer.write_all(format!("{{\"cmd\":\"focus_window\",\"id\":{firefox_id}}}\n").as_bytes()).unwrap();
            let ev = wait_for(&mut reader, |v| v["event"] == "windows" && v["windows"][0]["app_id"] == "firefox");
            assert!(windows(&ev)[0].3);
            firefox_id
        },
        |state, firefox_id| {
            // 4. Focusing a window on another workspace switches to it.
            let ff = state.focused_window().unwrap();
            assert_eq!(state.window_id(&ff), firefox_id);
            state.run_action(Action::MoveToWorkspace(4));
            assert_eq!(state.workspaces.active(), 0, "moving a window doesn't follow it");
            state.run_action(Action::FocusWindow(firefox_id));
            assert_eq!(state.workspaces.active(), 3, "focusing it switched to workspace 4");
            assert_eq!(state.focused_window().map(|w| state.window_id(&w)), Some(firefox_id));
            // Unknown ids are ignored.
            state.run_action(Action::FocusWindow(9999));
            assert_eq!(state.workspaces.active(), 3);
        },
    );
}
