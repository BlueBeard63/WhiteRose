//! Starts the compositor core headless (no backend), connects a real
//! Wayland client to its socket, and checks the protocols it advertises.

use std::{os::unix::net::UnixStream, path::PathBuf, time::Duration};

use smithay::reexports::{calloop::EventLoop, wayland_server::Display};
use wayland_client::{
    globals::{registry_queue_init, GlobalListContents},
    protocol::wl_registry,
    Connection, Dispatch, QueueHandle,
};
use whiterose_wm::{keybind::Bindings, WhiteRose};

struct Client;

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for Client {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

fn runtime_dir() -> PathBuf {
    // Never touch the user's real clipboard history from tests.
    if std::env::var_os("WHITEROSE_CLIPBOARD_DIR").is_none() {
        std::env::set_var(
            "WHITEROSE_CLIPBOARD_DIR",
            std::env::temp_dir().join(format!("whiterose-clip-test-{}", std::process::id())),
        );
    }
    // CI runners often have no XDG_RUNTIME_DIR; give the socket somewhere to live.
    if let Some(dir) = std::env::var_os("XDG_RUNTIME_DIR") {
        return dir.into();
    }
    let dir = std::env::temp_dir().join(format!("whiterose-test-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    std::env::set_var("XDG_RUNTIME_DIR", &dir);
    dir
}

#[test]
fn advertises_core_globals() {
    let runtime = runtime_dir();
    let mut event_loop: EventLoop<'static, WhiteRose> = EventLoop::try_new().unwrap();
    let display: Display<WhiteRose> = Display::new().unwrap();
    let mut state = WhiteRose::new(&mut event_loop, display, Bindings::defaults("true"));

    let socket = runtime.join(&state.socket_name);
    let client = std::thread::spawn(move || {
        let stream = UnixStream::connect(&socket).expect("connect to compositor socket");
        let conn = Connection::from_socket(stream).unwrap();
        let (globals, _queue) = registry_queue_init::<Client>(&conn).expect("registry roundtrip");
        globals.contents().clone_list().into_iter().map(|g| g.interface).collect::<Vec<_>>()
    });

    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while !client.is_finished() {
        assert!(std::time::Instant::now() < deadline, "client never finished its roundtrip");
        event_loop.dispatch(Some(Duration::from_millis(10)), &mut state).unwrap();
    }
    let interfaces = client.join().unwrap();

    for want in [
        "wl_compositor",
        "wl_shm",
        "xdg_wm_base",
        "wl_seat",
        "wl_data_device_manager",
        "zxdg_output_manager_v1",
        "zwlr_layer_shell_v1",
    ] {
        assert!(interfaces.iter().any(|i| i == want), "missing {want}; advertised: {interfaces:?}");
    }
}

#[test]
fn dmabuf_is_only_advertised_with_a_renderer() {
    // Headless has no GPU to import buffers into, so it must not offer
    // linux-dmabuf; clients then fall back to shared memory instead of failing.
    runtime_dir();
    let mut event_loop: EventLoop<'static, WhiteRose> = EventLoop::try_new().unwrap();
    let display: Display<WhiteRose> = Display::new().unwrap();
    let state = WhiteRose::new(&mut event_loop, display, Bindings::defaults("true"));
    assert!(state.dmabuf_global.is_none());
    assert!(state.dmabuf_importer.is_none());
}

#[test]
fn starts_on_first_workspace_with_nothing_focused() {
    runtime_dir();
    let mut event_loop: EventLoop<'static, WhiteRose> = EventLoop::try_new().unwrap();
    let display: Display<WhiteRose> = Display::new().unwrap();
    let state = WhiteRose::new(&mut event_loop, display, Bindings::defaults("true"));
    assert_eq!(state.workspaces.active(), 0);
    assert_eq!(state.workspaces.count(), whiterose_wm::state::WORKSPACES);
    assert!(state.focused_window().is_none());
    assert!(state.redraw.is_none(), "headless state must not try to draw");
}
