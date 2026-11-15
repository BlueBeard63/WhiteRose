//! End-to-end shell IPC: a client on the real socket against a headless compositor.

use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    sync::mpsc,
    time::{Duration, Instant},
};

use serde_json::Value;
use smithay::reexports::{calloop::EventLoop, wayland_server::Display};
use whiterose_wm::{
    keybind::{Action, Bindings},
    WhiteRose,
};

fn runtime_dir() {
    // Never touch the user's real clipboard history from tests.
    if std::env::var_os("WHITEROSE_CLIPBOARD_DIR").is_none() {
        std::env::set_var(
            "WHITEROSE_CLIPBOARD_DIR",
            std::env::temp_dir().join(format!("whiterose-clip-test-{}", std::process::id())),
        );
    }
    if std::env::var_os("XDG_RUNTIME_DIR").is_none() {
        let dir = std::env::temp_dir().join(format!("whiterose-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("XDG_RUNTIME_DIR", &dir);
    }
}

struct Shell {
    reader: BufReader<UnixStream>,
    writer: UnixStream,
}

impl Shell {
    fn connect(path: &std::path::Path) -> Shell {
        let stream = UnixStream::connect(path).expect("connect to ipc socket");
        stream.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        Shell { writer: stream.try_clone().unwrap(), reader: BufReader::new(stream) }
    }
    fn recv(&mut self) -> Value {
        let mut line = String::new();
        self.reader.read_line(&mut line).expect("event within 3s");
        assert!(line.ends_with('\n'), "events are newline-terminated: {line:?}");
        serde_json::from_str(&line).unwrap_or_else(|e| panic!("bad JSON {line:?}: {e}"))
    }
    fn send(&mut self, line: &str) {
        self.writer.write_all(line.as_bytes()).unwrap();
        self.writer.write_all(b"\n").unwrap();
    }
}

#[test]
fn shell_ipc_round_trip() {
    runtime_dir();
    let mut event_loop: EventLoop<'static, WhiteRose> = EventLoop::try_new().unwrap();
    let display: Display<WhiteRose> = Display::new().unwrap();
    let mut state = WhiteRose::new(&mut event_loop, display, Bindings::defaults("true"));
    let path = state.ipc.as_ref().expect("ipc socket created").path.clone();
    assert!(path.exists());

    let (want_action_tx, want_action_rx) = mpsc::channel::<()>();
    let client = std::thread::spawn(move || {
        let mut shell = Shell::connect(&path);

        // 1. Snapshot on connect.
        let ws = shell.recv();
        assert_eq!(ws["event"], "workspaces");
        assert_eq!(ws["active"], 1);
        assert_eq!(ws["count"], 9);
        assert_eq!(ws["occupied"], serde_json::json!([]));
        let focus = shell.recv();
        assert_eq!(focus["event"], "focus");
        assert_eq!(focus["app_id"], Value::Null);
        let windows = shell.recv();
        assert_eq!(windows, serde_json::json!({"event": "windows", "windows": []}));
        let island = shell.recv();
        assert_eq!(
            island,
            serde_json::json!({"event": "island", "covered": [], "fullscreen": []}),
            "nothing under the island"
        );
        let outputs = shell.recv();
        assert_eq!(outputs["event"], "outputs", "the Displays page learns the outputs");
        let lock = shell.recv();
        assert_eq!(lock, serde_json::json!({"event": "lock", "locked": false}), "a new shell learns the lock state");
        let shortcuts = shell.recv();
        assert_eq!(shortcuts["event"], "shortcuts", "Keyboard & shortcuts learns the shortcuts");
        let close = shortcuts["shortcuts"].as_array().unwrap().iter().find(|s| s["id"] == "close-window").unwrap();
        assert_eq!(close["keys"], serde_json::json!(["SUPER+Q"]));
        assert_eq!(shell.recv()["event"], "keyboard", "and the keyboard layouts");

        // 2. Garbage is ignored and the connection survives; duplicates aren't re-sent.
        shell.send("this is not json");
        shell.send(r#"{"cmd":"format_disk"}"#);
        shell.send(r#"{"cmd":"workspace","n":3}"#);
        shell.send(r#"{"cmd":"workspace","n":3}"#);
        shell.send(r#"{"cmd":"workspace","n":4}"#);
        let a = shell.recv();
        assert_eq!((a["event"].as_str(), a["active"].as_u64()), (Some("workspaces"), Some(3)));
        let b = shell.recv();
        assert_eq!(
            (b["event"].as_str(), b["active"].as_u64()),
            (Some("workspaces"), Some(4)),
            "duplicate state was re-sent"
        );

        // 3. Shell shortcuts arrive as action events.
        want_action_tx.send(()).unwrap();
        let act = shell.recv();
        assert_eq!(act, serde_json::json!({"event": "action", "name": "screenshot_region"}));
    });

    let deadline = Instant::now() + Duration::from_secs(5);
    let mut action_sent = false;
    while !client.is_finished() {
        assert!(Instant::now() < deadline, "ipc test timed out");
        event_loop.dispatch(Some(Duration::from_millis(5)), &mut state).unwrap();
        state.after_dispatch();
        if !action_sent && want_action_rx.try_recv().is_ok() {
            state.run_action(Action::ScreenshotRegion);
            action_sent = true;
        }
    }
    client.join().expect("client assertions");
    assert_eq!(state.workspaces.active(), 3, "workspace 4 is index 3");

    // 4. Disconnects are noticed and cleaned up.
    for _ in 0..20 {
        event_loop.dispatch(Some(Duration::from_millis(5)), &mut state).unwrap();
        state.after_dispatch();
        if state.ipc.as_ref().unwrap().client_count() == 0 {
            break;
        }
    }
    assert_eq!(state.ipc.as_ref().unwrap().client_count(), 0);
}

#[test]
fn socket_is_removed_on_shutdown() {
    runtime_dir();
    let mut event_loop: EventLoop<'static, WhiteRose> = EventLoop::try_new().unwrap();
    let display: Display<WhiteRose> = Display::new().unwrap();
    let state = WhiteRose::new(&mut event_loop, display, Bindings::defaults("true"));
    let path = state.ipc.as_ref().unwrap().path.clone();
    assert!(path.exists());
    drop(state);
    assert!(!path.exists(), "stale socket left behind at {}", path.display());
}

#[test]
fn shell_actions_without_a_shell_do_not_panic() {
    runtime_dir();
    let mut event_loop: EventLoop<'static, WhiteRose> = EventLoop::try_new().unwrap();
    let display: Display<WhiteRose> = Display::new().unwrap();
    let mut state = WhiteRose::new(&mut event_loop, display, Bindings::defaults("true"));
    for a in [Action::AppLauncher, Action::ScreenshotScreen, Action::AppSwitcher] {
        state.run_action(a);
    }
}

#[test]
fn shell_cannot_delete_files_the_compositor_did_not_save() {
    runtime_dir();
    let mut event_loop: EventLoop<'static, WhiteRose> = EventLoop::try_new().unwrap();
    let display: Display<WhiteRose> = Display::new().unwrap();
    let mut state = WhiteRose::new(&mut event_loop, display, Bindings::defaults("true"));

    let victim = std::env::temp_dir().join(format!("whiterose-not-a-screenshot-{}", std::process::id()));
    std::fs::write(&victim, b"precious").unwrap();
    state.run_action(Action::DeleteScreenshot(victim.to_string_lossy().into_owned()));
    assert!(victim.exists(), "an arbitrary file was deleted over IPC");

    // A file the compositor did save may be deleted.
    let ours = std::env::temp_dir().join(format!("whiterose-ours-{}.png", std::process::id()));
    std::fs::write(&ours, b"png").unwrap();
    state.saved_screenshots.push(ours.clone());
    state.run_action(Action::DeleteScreenshot(ours.to_string_lossy().into_owned()));
    assert!(!ours.exists());
    std::fs::remove_file(&victim).unwrap();
}

#[test]
fn screenshot_without_a_renderer_reports_failure_to_the_shell() {
    runtime_dir();
    let mut event_loop: EventLoop<'static, WhiteRose> = EventLoop::try_new().unwrap();
    let display: Display<WhiteRose> = Display::new().unwrap();
    let mut state = WhiteRose::new(&mut event_loop, display, Bindings::defaults("true"));
    let path = state.ipc.as_ref().unwrap().path.clone();

    let client = std::thread::spawn(move || {
        let mut shell = Shell::connect(&path);
        shell.recv(); // workspaces snapshot
        shell.recv(); // focus snapshot
        shell.recv(); // windows snapshot
        shell.recv(); // island snapshot
        shell.recv(); // outputs snapshot
        shell.recv(); // lock snapshot
        shell.recv(); // shortcuts snapshot
        shell.recv(); // keyboard snapshot
        shell.send(r#"{"cmd":"screenshot"}"#);
        shell.recv()
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !client.is_finished() {
        assert!(Instant::now() < deadline);
        event_loop.dispatch(Some(Duration::from_millis(5)), &mut state).unwrap();
        state.after_dispatch();
    }
    let event = client.join().unwrap();
    assert_eq!(event["event"], "screenshot_failed");
    assert!(event["error"].as_str().unwrap().contains("headless"));
    assert!(!state.screenshot_pending, "must not wait forever for a frame that never comes");
}
