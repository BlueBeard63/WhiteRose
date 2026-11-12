//! X11 windows (XWayland) are windows like any other: they tile, maximize,
//! snap, follow the rules, can be scratchpads, and the shell knows their
//! app id and title. Each test starts a real Xwayland, and is skipped where
//! there isn't one.

mod common;

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use common::{tiles::*, *};
use serde_json::json;
use smithay::desktop::Window;
use whiterose_wm::{win::WinExt, WhiteRose};
use x11rb::{
    connection::Connection as _,
    protocol::xproto::{AtomEnum, ConnectionExt as _, CreateWindowAux, PropMode, Window as XWindow, WindowClass},
    rust_connection::RustConnection,
    wrapper::ConnectionExt as _,
    COPY_DEPTH_FROM_PARENT,
};

const BTN_LEFT: u32 = 0x110;

/// A compositor with Xwayland running and the bar open; None (the test is
/// skipped) without an Xwayland.
fn with_x11(test: impl FnOnce(&mut Headless) + 'static) {
    if which("Xwayland").is_none() {
        eprintln!("skipped: Xwayland isn't installed");
        return;
    }
    let mut h = Headless::new(true);
    let handle = h.event_loop.handle();
    h.state.start_xwayland(&handle);
    let deadline = Instant::now() + Duration::from_secs(10);
    while h.state.xwayland.wm.is_none() {
        assert!(Instant::now() < deadline, "Xwayland didn't start");
        h.dispatch();
    }
    test(&mut h);
}

fn which(program: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH")?.to_str()?.split(':').map(|d| std::path::Path::new(d).join(program)).find(|p| p.exists())
}

/// An X11 client of the compositor's Xwayland.
struct X {
    conn: RustConnection,
    root: XWindow,
}

impl X {
    fn connect(ask: &Ask) -> X {
        let display = ask.run(|s| s.xwayland.display.unwrap());
        let (conn, screen) = x11rb::connect(Some(&format!(":{display}"))).expect("connected to Xwayland");
        let root = conn.setup().roots[screen].root;
        X { conn, root }
    }

    fn atom(&self, name: &str) -> u32 {
        self.conn.intern_atom(false, name.as_bytes()).unwrap().reply().unwrap().atom
    }

    /// Opens a 400x300 window with WM_CLASS ("x", `class`), `title`, this
    /// process's pid, and transient for `parent` if given.
    fn open(&self, class: &str, title: &str, parent: Option<XWindow>) -> XWindow {
        let c = &self.conn;
        let win = c.generate_id().unwrap();
        let aux = CreateWindowAux::new().background_pixel(0x00ff_8040);
        c.create_window(COPY_DEPTH_FROM_PARENT, win, self.root, 0, 0, 400, 300, 0, WindowClass::INPUT_OUTPUT, 0, &aux)
            .unwrap();
        c.change_property8(
            PropMode::REPLACE,
            win,
            AtomEnum::WM_CLASS,
            AtomEnum::STRING,
            format!("x\0{class}\0").as_bytes(),
        )
        .unwrap();
        c.change_property8(PropMode::REPLACE, win, AtomEnum::WM_NAME, AtomEnum::STRING, title.as_bytes()).unwrap();
        c.change_property32(
            PropMode::REPLACE,
            win,
            self.atom("_NET_WM_PID"),
            AtomEnum::CARDINAL,
            &[std::process::id()],
        )
        .unwrap();
        if let Some(p) = parent {
            c.change_property32(PropMode::REPLACE, win, AtomEnum::WM_TRANSIENT_FOR, AtomEnum::WINDOW, &[p]).unwrap();
        }
        c.map_window(win).unwrap();
        c.flush().unwrap();
        win
    }

    fn set_title(&self, win: XWindow, title: &str) {
        self.conn
            .change_property8(PropMode::REPLACE, win, AtomEnum::WM_NAME, AtomEnum::STRING, title.as_bytes())
            .unwrap();
        self.conn.flush().unwrap();
    }

    /// Where X has `win` (on the root) and its size.
    fn rect(&self, win: XWindow) -> (i32, i32, i32, i32) {
        let at = self.conn.translate_coordinates(win, self.root, 0, 0).unwrap().reply().unwrap();
        let g = self.conn.get_geometry(win).unwrap().reply().unwrap();
        (at.dst_x as i32, at.dst_y as i32, g.width as i32, g.height as i32)
    }

    /// Waits until X has `win` at `want`.
    fn until_rect(&self, win: XWindow, want: (i32, i32, i32, i32)) {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let got = self.rect(win);
            if got == want {
                return;
            }
            assert!(Instant::now() < deadline, "X has the window at {got:?}, not {want:?}");
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

/// Asks the compositor until `done` (5 s at most).
fn until(ask: &Ask, what: &str, done: impl Fn(&mut WhiteRose) -> bool + Send + Sync + 'static) {
    let done = Arc::new(done);
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let d = done.clone();
        if ask.run(move |s| d(s)) {
            return;
        }
        assert!(Instant::now() < deadline, "never: {what}");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Every window, shown or hidden, with WM_CLASS `class`.
fn by_class(s: &WhiteRose, class: &str) -> Option<Window> {
    s.space.elements().chain(s.hidden_at.iter().map(|(w, _)| w)).find(|w| w.ids().0.as_deref() == Some(class)).cloned()
}

/// Waits for `class` to open and draw (its first commit: rules and tiles).
fn opened(ask: &Ask, class: &'static str) {
    until(ask, &format!("{class} opens"), move |s| by_class(s, class).is_some_and(|w| w.surface().is_some()));
    ask.run(|s| s.after_dispatch());
}

/// Tiled on, two X11 windows tile side by side, and X is told where.
fn two_x11_tiles(ask: &Ask) -> (X, XWindow, XWindow) {
    let x = X::connect(ask);
    let a = x.open("One", "one", None);
    opened(ask, "One");
    let b = x.open("Two", "two", None);
    opened(ask, "Two");
    until(ask, "they tile", |s| {
        let (a, b) = (by_class(s, "One").unwrap(), by_class(s, "Two").unwrap());
        rect(s, &a) == Some(LEFT) && rect(s, &b) == Some(RIGHT)
    });
    x.until_rect(a, LEFT);
    x.until_rect(b, RIGHT);
    (x, a, b)
}

#[test]
fn x11_windows_tile_and_x_is_told_where() {
    with_x11(|h| {
        set(&mut h.state, "windows.layout", json!("dwindle"));
        h.drive(|conn, ask| {
            let mut c = Client::connect(&conn);
            c.open_bar();
            let (x, a, b) = two_x11_tiles(&ask);
            ask.run(|s| assert!(told_tiled(&by_class(s, "One").unwrap()), "it knows it's tiled"));
            // Closing one gives its space back.
            x.conn.destroy_window(a).unwrap();
            x.conn.flush().unwrap();
            until(&ask, "One closes", |s| by_class(s, "One").is_none());
            x.until_rect(b, AREA);
        });
    });
}

#[test]
fn an_x11_dialog_floats_above_the_tiles() {
    with_x11(|h| {
        set(&mut h.state, "windows.layout", json!("dwindle"));
        h.drive(|conn, ask| {
            let mut c = Client::connect(&conn);
            c.open_bar();
            let x = X::connect(&ask);
            let main = x.open("Main", "main", None);
            opened(&ask, "Main");
            x.open("Ask", "ask", Some(main));
            opened(&ask, "Ask");
            ask.run(|s| {
                let (m, d) = (by_class(s, "Main").unwrap(), by_class(s, "Ask").unwrap());
                assert!(s.is_tiled(&m));
                assert!(!s.is_tiled(&d), "the dialog floats");
                assert_eq!(rect(s, &m), Some(AREA));
            });
        });
    });
}

#[test]
fn an_x11_tile_maximizes_and_comes_back() {
    with_x11(|h| {
        set(&mut h.state, "windows.layout", json!("dwindle"));
        h.drive(|conn, ask| {
            let mut c = Client::connect(&conn);
            c.open_bar();
            let (x, a, _b) = two_x11_tiles(&ask);
            ask.run(|s| s.maximize(&by_class(s, "One").unwrap()));
            x.until_rect(a, (0, 26, 1440, 782));
            ask.run(|s| s.unmaximize(&by_class(s, "One").unwrap()));
            x.until_rect(a, LEFT);
        });
    });
}

#[test]
fn an_x11_window_snaps_to_a_half() {
    with_x11(|h| {
        h.drive(|conn, ask| {
            let mut c = Client::connect(&conn);
            c.open_bar();
            let x = X::connect(&ask);
            let a = x.open("Snap", "snap", None);
            opened(&ask, "Snap");
            ask.run(|s| {
                let w = by_class(s, "Snap").unwrap();
                let (lx, ly, _, _) = rect(s, &w).unwrap();
                let from = (lx as f64 + 50.0, ly as f64 + 50.0);
                super_drag(s, BTN_LEFT, from, (0.0, 400.0));
                assert!(s.snap_of(&w).is_some());
                assert_eq!(rect(s, &w), Some(LEFT));
            });
            x.until_rect(a, LEFT);
        });
    });
}

#[test]
fn rules_match_an_x11_window_by_its_class() {
    with_x11(|h| {
        h.state.rules_setting("windows.rules", &json!([{"app": "^Chat$", "workspace": 3}]));
        h.drive(|conn, ask| {
            let mut c = Client::connect(&conn);
            c.open_bar();
            let x = X::connect(&ask);
            x.open("Chat", "chat", None);
            until(&ask, "Chat goes to workspace 3", |s| {
                by_class(s, "Chat").is_some_and(|w| s.workspaces.workspace_of(&w) == Some(2))
            });
            ask.run(|s| assert_eq!(s.workspaces.active(), 0, "its workspace doesn't show"));
        });
    });
}

#[test]
fn an_x11_app_can_be_a_scratchpad() {
    with_x11(|h| {
        h.state.scratchpads.slide_length = Duration::ZERO;
        let pad = json!([{"id": "term", "name": "Terminal", "command": "true", "keys": ["SUPER+GRAVE"]}]);
        if h.state.scratchpads_setting("windows.scratchpads", &pad) {
            h.state.apply_shortcuts();
        }
        h.drive(|conn, ask| {
            let mut c = Client::connect(&conn);
            c.open_bar();
            let x = X::connect(&ask);
            ask.run(|s| s.expect_scratchpad("term", std::process::id()));
            x.open("Pad", "pad", None);
            until(&ask, "Pad is the scratchpad", |s| by_class(s, "Pad").is_some_and(|w| s.scratchpads.is_pad(&w)));
            ask.run(|s| assert_eq!(s.workspaces.workspace_of(&by_class(s, "Pad").unwrap()), None, "on no workspace"));
        });
    });
}

#[test]
fn the_shell_sees_an_x11_windows_class_title_and_focus() {
    with_x11(|h| {
        h.drive(|conn, ask| {
            let mut c = Client::connect(&conn);
            c.open_bar();
            let x = X::connect(&ask);
            let a = x.open("Spotify", "Spotify Free", None);
            opened(&ask, "Spotify");
            ask.run(|s| {
                let w = by_class(s, "Spotify").unwrap();
                assert_eq!(whiterose_wm::ipc::toplevel_ids(&w), (Some("Spotify".into()), Some("Spotify Free".into())));
                assert_eq!(s.focused_window(), Some(w.clone()));
                assert!(
                    w.has(smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State::Activated)
                );
            });
            x.set_title(a, "Song - Artist");
            until(&ask, "the new title", |s| {
                by_class(s, "Spotify").is_some_and(|w| w.ids().1.as_deref() == Some("Song - Artist"))
            });
        });
    });
}
