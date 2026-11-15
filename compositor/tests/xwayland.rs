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

use std::{
    io::Read,
    os::unix::net::UnixStream,
    sync::atomic::{AtomicBool, Ordering},
};
use wayland_client::{
    delegate_noop,
    protocol::{
        wl_data_device::{self, WlDataDevice},
        wl_data_device_manager::WlDataDeviceManager,
        wl_data_offer::WlDataOffer,
        wl_data_source::{self, WlDataSource},
        wl_seat::WlSeat,
    },
    Connection, Dispatch, QueueHandle,
};
use x11rb::protocol::{
    xproto::{Atom, EventMask, SelectionNotifyEvent, SELECTION_NOTIFY_EVENT},
    Event,
};

const BTN_LEFT: u32 = 0x110;

/// A compositor taking X11 apps (Xwayland starts with the first that
/// connects) with the bar open; the test is skipped without an Xwayland.
fn with_x11(test: impl FnOnce(&mut Headless) + 'static) {
    if which("Xwayland").is_none() {
        eprintln!("skipped: Xwayland isn't installed");
        return;
    }
    let mut h = Headless::new(true);
    h.state.listen_xwayland();
    assert!(h.state.xwayland.display.is_some(), "an X display is taken");
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

// ---------------------------------------------------------------- the clipboard

const TEXT: &str = "text/plain;charset=utf-8";

/// What a copying Wayland app hands over when asked.
struct Payload(&'static [u8]);

impl Dispatch<WlDataSource, Payload> for App {
    fn event(
        _: &mut Self,
        _: &WlDataSource,
        event: wl_data_source::Event,
        data: &Payload,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_data_source::Event::Send { fd, .. } = event {
            use std::io::Write;
            let _ = std::fs::File::from(fd).write_all(data.0);
        }
    }
}
impl Dispatch<WlDataDevice, ()> for App {
    fn event(_: &mut Self, _: &WlDataDevice, _: wl_data_device::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
    wayland_client::event_created_child!(App, WlDataDevice, [
        wl_data_device::EVT_DATA_OFFER_OPCODE => (WlDataOffer, ()),
    ]);
}
delegate_noop!(App: ignore WlDataDeviceManager);
delegate_noop!(App: ignore WlDataOffer);

/// A Wayland app (with the keyboard) copies `data` as text.
fn wayland_copy(c: &mut Client, conn: &Connection, data: &'static [u8]) {
    let (globals, _) = wayland_client::globals::registry_queue_init::<App>(conn).unwrap();
    let qh = c.queue.handle();
    let manager: WlDataDeviceManager = globals.bind(&qh, 1..=3, ()).unwrap();
    let seat: WlSeat = globals.bind(&qh, 1..=7, ()).unwrap();
    let device = manager.get_data_device(&seat, &qh, ());
    let source = manager.create_data_source(&qh, Payload(data));
    source.offer(TEXT.into());
    device.set_selection(Some(&source), 0);
    c.queue.roundtrip(&mut c.app).unwrap();
    std::mem::forget((device, source));
}

/// An X11 app pastes, as UTF8_STRING, on its own connection (and thread):
/// what it got, or None if it was refused.
fn x11_paste(display: u32) -> std::thread::JoinHandle<Option<Vec<u8>>> {
    std::thread::spawn(move || {
        let (conn, screen) = x11rb::connect(Some(&format!(":{display}"))).unwrap();
        let root = conn.setup().roots[screen].root;
        let win = conn.generate_id().unwrap();
        conn.create_window(
            COPY_DEPTH_FROM_PARENT,
            win,
            root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new(),
        )
        .unwrap();
        let atom = |n: &str| conn.intern_atom(false, n.as_bytes()).unwrap().reply().unwrap().atom;
        let (clipboard, utf8, prop) = (atom("CLIPBOARD"), atom("UTF8_STRING"), atom("WR_PASTE"));
        conn.convert_selection(win, clipboard, utf8, prop, x11rb::CURRENT_TIME).unwrap();
        conn.flush().unwrap();
        let deadline = Instant::now() + Duration::from_secs(4);
        while Instant::now() < deadline {
            match conn.poll_for_event().unwrap() {
                Some(Event::SelectionNotify(n)) => {
                    if n.property == x11rb::NONE {
                        return None;
                    }
                    let reply = conn.get_property(true, win, prop, AtomEnum::ANY, 0, 1 << 20).unwrap().reply().unwrap();
                    return Some(reply.value);
                }
                Some(_) => {}
                None => std::thread::sleep(Duration::from_millis(5)),
            }
        }
        panic!("no answer to the paste");
    })
}

/// An X11 app copies `data` (owning CLIPBOARD) and hands it over until
/// `stop`, on its own connection and thread.
fn x11_copy(display: u32, data: &'static [u8], stop: Arc<AtomicBool>) -> std::thread::JoinHandle<()> {
    std::thread::spawn(move || {
        let (conn, screen) = x11rb::connect(Some(&format!(":{display}"))).unwrap();
        let root = conn.setup().roots[screen].root;
        let win = conn.generate_id().unwrap();
        conn.create_window(
            COPY_DEPTH_FROM_PARENT,
            win,
            root,
            0,
            0,
            1,
            1,
            0,
            WindowClass::INPUT_OUTPUT,
            0,
            &CreateWindowAux::new(),
        )
        .unwrap();
        let atom = |n: &str| conn.intern_atom(false, n.as_bytes()).unwrap().reply().unwrap().atom;
        let (clipboard, targets, utf8, text) = (atom("CLIPBOARD"), atom("TARGETS"), atom("UTF8_STRING"), atom(TEXT));
        conn.set_selection_owner(win, clipboard, x11rb::CURRENT_TIME).unwrap();
        conn.flush().unwrap();
        while !stop.load(Ordering::Relaxed) {
            let Some(event) = conn.poll_for_event().unwrap() else {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            };
            let Event::SelectionRequest(r) = event else { continue };
            if r.target == targets {
                let list: [Atom; 3] = [targets, utf8, text];
                conn.change_property32(PropMode::REPLACE, r.requestor, r.property, AtomEnum::ATOM, &list).unwrap();
            } else {
                conn.change_property8(PropMode::REPLACE, r.requestor, r.property, r.target, data).unwrap();
            }
            let notify = SelectionNotifyEvent {
                response_type: SELECTION_NOTIFY_EVENT,
                sequence: 0,
                time: r.time,
                requestor: r.requestor,
                selection: r.selection,
                target: r.target,
                property: r.property,
            };
            conn.send_event(false, r.requestor, EventMask::NO_EVENT, notify).unwrap();
            conn.flush().unwrap();
        }
    })
}

/// Waits for a paste thread while the Wayland client keeps answering.
fn pasted(c: &mut Client, paste: std::thread::JoinHandle<Option<Vec<u8>>>) -> Option<String> {
    while !paste.is_finished() {
        c.queue.roundtrip(&mut c.app).unwrap();
        std::thread::sleep(Duration::from_millis(5));
    }
    paste.join().unwrap().map(|b| String::from_utf8(b).unwrap())
}

#[test]
fn a_wayland_copy_pastes_into_an_x11_app() {
    with_x11(|h| {
        h.drive(|conn, ask| {
            let mut c = Client::connect(&conn);
            c.open_window(); // apps need the keyboard to copy
            wayland_copy(&mut c, &conn, b"from wayland");
            let x = X::connect(&ask);
            x.open("Paste", "paste", None);
            opened(&ask, "Paste");
            let display = ask.run(|s| s.xwayland.display.unwrap());
            assert_eq!(pasted(&mut c, x11_paste(display)).as_deref(), Some("from wayland"));
        });
    });
}

#[test]
fn the_compositors_own_clipboard_pastes_into_an_x11_app() {
    with_x11(|h| {
        h.drive(|conn, ask| {
            let mut c = Client::connect(&conn);
            c.open_bar();
            let x = X::connect(&ask);
            x.open("Paste", "paste", None);
            opened(&ask, "Paste");
            // As a history entry pasted back, or a screenshot.
            ask.run(|s| s.offer_clipboard(vec![(TEXT.into(), b"from the history".to_vec())]));
            let display = ask.run(|s| s.xwayland.display.unwrap());
            assert_eq!(pasted(&mut c, x11_paste(display)).as_deref(), Some("from the history"));
        });
    });
}

#[test]
fn an_x11_copy_pastes_into_wayland_apps_and_is_remembered() {
    with_x11(|h| {
        h.state.clipboard_clear();
        h.drive(|conn, ask| {
            let mut c = Client::connect(&conn);
            c.open_bar();
            let x = X::connect(&ask);
            x.open("Copy", "copy", None);
            opened(&ask, "Copy");
            let display = ask.run(|s| s.xwayland.display.unwrap());
            let stop = Arc::new(AtomicBool::new(false));
            let copier = x11_copy(display, b"from x11", stop.clone());
            until(&ask, "the X11 copy is in the history", |s| {
                s.clipboard.entries.iter().any(
                    |e| matches!(&e.content, whiterose_wm::clipboard::Content::Text { text } if text == "from x11"),
                )
            });
            // A Wayland app pasting asks the compositor, which asks X.
            let reader = ask.run(|s| {
                let (r, w) = UnixStream::pair().unwrap();
                assert!(s.read_clipboard(TEXT.into(), w.into()));
                r
            });
            let mut got = String::new();
            reader.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
            (&reader).read_to_string(&mut got).unwrap();
            assert_eq!(got, "from x11");
            stop.store(true, Ordering::Relaxed);
            copier.join().unwrap();
        });
    });
}

#[test]
fn an_x11_app_without_the_keyboard_cant_read_the_clipboard() {
    with_x11(|h| {
        h.drive(|conn, ask| {
            let mut c = Client::connect(&conn);
            c.open_bar();
            let x = X::connect(&ask);
            x.open("Snoop", "snoop", None);
            opened(&ask, "Snoop");
            // A Wayland window takes the keyboard.
            let (w, _t) = open(&mut c);
            c.attach_buffer(&w, 400, 300);
            until(&ask, "the Wayland window is focused", |s| {
                s.focused_window().is_some_and(|w| w.x11_surface().is_none())
            });
            ask.run(|s| s.offer_clipboard(vec![(TEXT.into(), b"secret".to_vec())]));
            let display = ask.run(|s| s.xwayland.display.unwrap());
            assert_eq!(pasted(&mut c, x11_paste(display)), None);
        });
    });
}

// ---------------------------------------------------------------- on demand

#[test]
fn xwayland_starts_with_the_first_x11_app() {
    with_x11(|h| {
        for _ in 0..50 {
            h.dispatch();
        }
        assert!(!h.state.xwayland.running(), "no X server until an X11 app wants one");
        h.drive(|conn, ask| {
            let mut c = Client::connect(&conn);
            c.open_bar();
            let x = X::connect(&ask);
            x.open("First", "first", None);
            opened(&ask, "First");
            ask.run(|s| assert!(s.xwayland.running() && s.xwayland.wm.is_some()));
        });
    });
}

#[test]
fn xwayland_exits_after_the_last_x11_app_and_comes_back_for_the_next() {
    with_x11(|h| {
        h.state.xwayland.linger = 1;
        h.drive(|conn, ask| {
            let mut c = Client::connect(&conn);
            c.open_bar();
            let x = X::connect(&ask);
            x.open("Once", "once", None);
            opened(&ask, "Once");
            drop(x);
            until(&ask, "Xwayland exits and its window goes", |s| {
                !s.xwayland.running() && by_class(s, "Once").is_none()
            });
            let x = X::connect(&ask);
            x.open("Again", "again", None);
            opened(&ask, "Again");
            ask.run(|s| assert!(s.xwayland.running()));
        });
    });
}

/// The pid of the Xwayland serving display `n` (this test's own).
fn xwayland_pid(n: u32) -> Option<u32> {
    std::fs::read_dir("/proc").ok()?.flatten().find_map(|e| {
        let pid: u32 = e.file_name().to_str()?.parse().ok()?;
        let cmd = std::fs::read(e.path().join("cmdline")).ok()?;
        let args: Vec<&[u8]> = cmd.split(|b| *b == 0).collect();
        let ours =
            args.first().is_some_and(|a| a.ends_with(b"Xwayland")) && args.get(1) == Some(&format!(":{n}").as_bytes());
        ours.then_some(pid)
    })
}

#[test]
fn a_crashed_xwayland_starts_again_for_the_next_x11_app() {
    with_x11(|h| {
        h.drive(|conn, ask| {
            let mut c = Client::connect(&conn);
            c.open_bar();
            let x = X::connect(&ask);
            x.open("Doomed", "doomed", None);
            opened(&ask, "Doomed");
            let n = ask.run(|s| s.xwayland.display.unwrap());
            let pid = xwayland_pid(n).expect("its Xwayland");
            std::process::Command::new("kill").arg("-9").arg(pid.to_string()).status().unwrap();
            drop(x);
            until(&ask, "its window goes", |s| !s.xwayland.running() && by_class(s, "Doomed").is_none());
            let x = X::connect(&ask);
            x.open("Again", "again", None);
            opened(&ask, "Again");
        });
    });
}

// ---------------------------------------------------------------- drag and drop

mod drag {
    //! A Wayland app dragging: its pointer, data device and source, on
    //! their own queue.
    use wayland_client::{
        globals::GlobalListContents,
        protocol::{
            wl_data_device::{self, WlDataDevice},
            wl_data_device_manager::WlDataDeviceManager,
            wl_data_offer::WlDataOffer,
            wl_data_source::{self, WlDataSource},
            wl_pointer::{self, WlPointer},
            wl_registry::WlRegistry,
            wl_seat::WlSeat,
            wl_surface::WlSurface,
        },
        Connection, Dispatch, QueueHandle,
    };

    #[derive(Default)]
    pub struct Drag {
        pub pointer_on: Option<WlSurface>,
        pub button_serial: Option<u32>,
        /// What the drop target accepts (the source's `target` events).
        pub target: Option<Option<String>>,
        /// Mime types the drop target asked for.
        pub sent: Vec<String>,
        pub finished: bool,
        pub cancelled: bool,
        pub payload: &'static [u8],
    }

    impl Dispatch<WlRegistry, GlobalListContents> for Drag {
        fn event(
            _: &mut Self,
            _: &WlRegistry,
            _: wayland_client::protocol::wl_registry::Event,
            _: &GlobalListContents,
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }
    impl Dispatch<WlSeat, ()> for Drag {
        fn event(
            _: &mut Self,
            _: &WlSeat,
            _: wayland_client::protocol::wl_seat::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }
    impl Dispatch<WlDataDeviceManager, ()> for Drag {
        fn event(
            _: &mut Self,
            _: &WlDataDeviceManager,
            _: <WlDataDeviceManager as wayland_client::Proxy>::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }
    impl Dispatch<WlPointer, ()> for Drag {
        fn event(d: &mut Self, _: &WlPointer, event: wl_pointer::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
            match event {
                wl_pointer::Event::Enter { surface, .. } => d.pointer_on = Some(surface),
                wl_pointer::Event::Leave { .. } => d.pointer_on = None,
                wl_pointer::Event::Button { serial, .. } => d.button_serial = Some(serial),
                _ => {}
            }
        }
    }
    impl Dispatch<WlDataDevice, ()> for Drag {
        fn event(
            _: &mut Self,
            _: &WlDataDevice,
            _: wl_data_device::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
        wayland_client::event_created_child!(Drag, WlDataDevice, [
            wl_data_device::EVT_DATA_OFFER_OPCODE => (WlDataOffer, ()),
        ]);
    }
    impl Dispatch<WlDataOffer, ()> for Drag {
        fn event(
            _: &mut Self,
            _: &WlDataOffer,
            _: wayland_client::protocol::wl_data_offer::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }
    impl Dispatch<WlDataSource, ()> for Drag {
        fn event(
            d: &mut Self,
            _: &WlDataSource,
            event: wl_data_source::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            match event {
                wl_data_source::Event::Target { mime_type } => d.target = Some(mime_type),
                wl_data_source::Event::Send { mime_type, fd } => {
                    use std::io::Write;
                    let _ = std::fs::File::from(fd).write_all(d.payload);
                    d.sent.push(mime_type);
                }
                wl_data_source::Event::DndFinished => d.finished = true,
                wl_data_source::Event::Cancelled => d.cancelled = true,
                _ => {}
            }
        }
    }
}

const URIS: &[u8] = b"file:///home/me/Pictures/cat.png\r\n";

/// An X11 app that takes drops (XdndAware), accepting a copy, on its own
/// connection and thread: the types it was offered and what it got.
fn x11_drop_target(display: u32) -> std::thread::JoinHandle<(Vec<String>, Vec<u8>)> {
    std::thread::spawn(move || {
        let (conn, screen) = x11rb::connect(Some(&format!(":{display}"))).unwrap();
        let root = conn.setup().roots[screen].root;
        let atom = |n: &str| conn.intern_atom(false, n.as_bytes()).unwrap().reply().unwrap().atom;
        let name = |a: Atom| String::from_utf8(conn.get_atom_name(a).unwrap().reply().unwrap().name).unwrap();
        let (aware, enter, position, status, drop, finished, selection, copy) = (
            atom("XdndAware"),
            atom("XdndEnter"),
            atom("XdndPosition"),
            atom("XdndStatus"),
            atom("XdndDrop"),
            atom("XdndFinished"),
            atom("XdndSelection"),
            atom("XdndActionCopy"),
        );
        let win = conn.generate_id().unwrap();
        let aux = CreateWindowAux::new().background_pixel(0x0040_80ff);
        conn.create_window(COPY_DEPTH_FROM_PARENT, win, root, 0, 0, 400, 300, 0, WindowClass::INPUT_OUTPUT, 0, &aux)
            .unwrap();
        conn.change_property8(PropMode::REPLACE, win, AtomEnum::WM_CLASS, AtomEnum::STRING, b"x\0Target\0").unwrap();
        conn.change_property8(PropMode::REPLACE, win, AtomEnum::WM_NAME, AtomEnum::STRING, b"target").unwrap();
        conn.change_property32(PropMode::REPLACE, win, aware, AtomEnum::ATOM, &[5]).unwrap();
        conn.map_window(win).unwrap();
        conn.flush().unwrap();
        let send = |to: u32, kind: Atom, data: [u32; 5]| {
            let ev = x11rb::protocol::xproto::ClientMessageEvent::new(32, to, kind, data);
            conn.send_event(false, to, EventMask::NO_EVENT, ev).unwrap();
            conn.flush().unwrap();
        };
        let mut types = Vec::new();
        let mut source = 0;
        let deadline = Instant::now() + Duration::from_secs(8);
        while Instant::now() < deadline {
            let Some(event) = conn.poll_for_event().unwrap() else {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            };
            match event {
                Event::ClientMessage(m) if m.type_ == enter => {
                    let d = m.data.as_data32();
                    source = d[0];
                    types = d[2..5].iter().filter(|a| **a != 0).map(|a| name(*a)).collect();
                }
                Event::ClientMessage(m) if m.type_ == position => {
                    send(m.data.as_data32()[0], status, [win, 1, 0, 0, copy]);
                }
                Event::ClientMessage(m) if m.type_ == drop => {
                    let time = m.data.as_data32()[2];
                    conn.convert_selection(win, selection, atom("text/uri-list"), atom("WR_DROP"), time).unwrap();
                    conn.flush().unwrap();
                }
                Event::SelectionNotify(n) if n.property != x11rb::NONE => {
                    let got =
                        conn.get_property(true, win, n.property, AtomEnum::ANY, 0, 1 << 20).unwrap().reply().unwrap();
                    send(source, finished, [win, 1, copy, 0, 0]);
                    // (Before it goes: closing at once can lose the message.)
                    conn.sync().unwrap();
                    return (types, got.value);
                }
                _ => {}
            }
        }
        panic!("no drop (offered {types:?})");
    })
}

#[test]
fn a_wayland_app_drags_a_file_onto_an_x11_app() {
    use wayland_client::protocol::{
        wl_data_device_manager::DndAction, wl_data_device_manager::WlDataDeviceManager, wl_seat::WlSeat,
    };
    with_x11(|h| {
        set(&mut h.state, "windows.layout", json!("dwindle"));
        h.drive(|conn, ask| {
            let mut c = Client::connect(&conn);
            c.open_bar();
            let (window, _t) = open(&mut c);
            c.attach_buffer(&window, LEFT.2, LEFT.3);
            // The X11 app takes the right half.
            X::connect(&ask);
            let display = ask.run(|s| s.xwayland.display.unwrap());
            let target = x11_drop_target(display);
            opened(&ask, "Target");
            until(&ask, "they tile", |s| by_class(s, "Target").is_some_and(|w| rect(s, &w) == Some(RIGHT)));

            let (globals, mut q) = wayland_client::globals::registry_queue_init::<drag::Drag>(&conn).unwrap();
            let qh = q.handle();
            let seat: WlSeat = globals.bind(&qh, 1..=7, ()).unwrap();
            let manager: WlDataDeviceManager = globals.bind(&qh, 3..=3, ()).unwrap();
            let _pointer = seat.get_pointer(&qh, ());
            let device = manager.get_data_device(&seat, &qh, ());
            let mut d = drag::Drag { payload: URIS, ..Default::default() };
            let wait = |q: &mut wayland_client::EventQueue<drag::Drag>,
                        d: &mut drag::Drag,
                        what: &str,
                        done: &dyn Fn(&drag::Drag) -> bool| {
                let deadline = Instant::now() + Duration::from_secs(5);
                while !done(d) {
                    assert!(Instant::now() < deadline, "never: {what}");
                    q.roundtrip(d).unwrap();
                    std::thread::sleep(Duration::from_millis(10));
                }
            };
            let time = Arc::new(std::sync::atomic::AtomicU32::new(0));
            let at = |x: i32, y: i32| (x as f64 + 0.5, y as f64 + 0.5);
            let mv = |p: (f64, f64)| {
                let t = time.fetch_add(10, Ordering::Relaxed);
                ask.run(move |s| s.on_pointer_motion(p.into(), t));
            };
            let button = |pressed: bool| {
                let t = time.fetch_add(10, Ordering::Relaxed);
                let state = if pressed {
                    smithay::backend::input::ButtonState::Pressed
                } else {
                    smithay::backend::input::ButtonState::Released
                };
                ask.run(move |s| s.on_pointer_button(BTN_LEFT, state, t));
            };

            // Press on the Wayland window and start dragging a file.
            q.roundtrip(&mut d).unwrap();
            mv(at(LEFT.0 + LEFT.2 / 2, LEFT.1 + LEFT.3 / 2));
            wait(&mut q, &mut d, "the pointer on the Wayland window", &|d| d.pointer_on.as_ref() == Some(&window));
            button(true);
            wait(&mut q, &mut d, "the press", &|d| d.button_serial.is_some());
            let source = manager.create_data_source(&qh, ());
            source.offer("text/uri-list".into());
            source.offer("text/plain;charset=utf-8".into());
            source.set_actions(DndAction::Copy);
            device.start_drag(Some(&source), &window, None, d.button_serial.unwrap());
            q.roundtrip(&mut d).unwrap();

            // Over the X11 app, which takes it.
            for step in 0..4 {
                // (Inside the 400x300 it drew before it was tiled.)
                mv(at(RIGHT.0 + 100 + step * 5, RIGHT.1 + 100));
                std::thread::sleep(Duration::from_millis(30));
            }
            wait(&mut q, &mut d, "the X11 app to accept", &|d| matches!(d.target, Some(Some(_))));

            // Drop: the X11 app reads the file list and says it's done.
            button(false);
            wait(&mut q, &mut d, "dnd_finished", &|d| d.finished || d.cancelled);
            assert!(d.finished, "the Wayland app heard the drop finished, not cancelled");
            assert_eq!(d.sent, ["text/uri-list"]);
            let (types, got) = target.join().unwrap();
            assert_eq!(types, ["text/uri-list", "text/plain;charset=utf-8"]);
            assert_eq!(got, URIS);
        });
    });
}

mod drop_target {
    //! A Wayland app taking drops: it accepts a file list as a copy.
    use wayland_client::{
        globals::GlobalListContents,
        protocol::{
            wl_data_device::{self, WlDataDevice},
            wl_data_device_manager::{DndAction, WlDataDeviceManager},
            wl_data_offer::{self, WlDataOffer},
            wl_registry::WlRegistry,
            wl_seat::WlSeat,
        },
        Connection, Dispatch, QueueHandle,
    };

    #[derive(Default)]
    pub struct DropTarget {
        pub offers: Vec<(WlDataOffer, Vec<String>)>,
        pub entered: Option<WlDataOffer>,
        pub dropped: bool,
    }

    impl Dispatch<WlRegistry, GlobalListContents> for DropTarget {
        fn event(
            _: &mut Self,
            _: &WlRegistry,
            _: wayland_client::protocol::wl_registry::Event,
            _: &GlobalListContents,
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }
    impl Dispatch<WlSeat, ()> for DropTarget {
        fn event(
            _: &mut Self,
            _: &WlSeat,
            _: wayland_client::protocol::wl_seat::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }
    impl Dispatch<WlDataDeviceManager, ()> for DropTarget {
        fn event(
            _: &mut Self,
            _: &WlDataDeviceManager,
            _: <WlDataDeviceManager as wayland_client::Proxy>::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
        }
    }
    impl Dispatch<WlDataDevice, ()> for DropTarget {
        fn event(
            d: &mut Self,
            _: &WlDataDevice,
            event: wl_data_device::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            match event {
                wl_data_device::Event::DataOffer { id } => d.offers.push((id, Vec::new())),
                wl_data_device::Event::Enter { serial, id: Some(offer), .. } => {
                    offer.accept(serial, Some("text/uri-list".into()));
                    offer.set_actions(DndAction::Copy, DndAction::Copy);
                    d.entered = Some(offer);
                }
                wl_data_device::Event::Leave => d.entered = None,
                wl_data_device::Event::Drop => d.dropped = true,
                _ => {}
            }
        }
        wayland_client::event_created_child!(DropTarget, WlDataDevice, [
            wl_data_device::EVT_DATA_OFFER_OPCODE => (WlDataOffer, ()),
        ]);
    }
    impl Dispatch<WlDataOffer, ()> for DropTarget {
        fn event(
            d: &mut Self,
            offer: &WlDataOffer,
            event: wl_data_offer::Event,
            _: &(),
            _: &Connection,
            _: &QueueHandle<Self>,
        ) {
            if let wl_data_offer::Event::Offer { mime_type } = event {
                if let Some((_, mimes)) = d.offers.iter_mut().find(|(o, _)| o == offer) {
                    mimes.push(mime_type);
                }
            }
        }
    }
}

/// An X11 app dragging a file list, as X11 apps do: while button 1 is held
/// it takes XdndSelection and talks XDND to the top-level window under the
/// pointer that takes drops, drops on release if that window accepted, and
/// hands over the data. Whether its drop finished well (None: no drop).
fn x11_drag_source(display: u32) -> std::thread::JoinHandle<Option<bool>> {
    std::thread::spawn(move || {
        use x11rb::protocol::xproto::KeyButMask;
        let (conn, screen) = x11rb::connect(Some(&format!(":{display}"))).unwrap();
        let root = conn.setup().roots[screen].root;
        let atom = |n: &str| conn.intern_atom(false, n.as_bytes()).unwrap().reply().unwrap().atom;
        let (aware, enter, position, status, leave, drop, finished, selection, copy) = (
            atom("XdndAware"),
            atom("XdndEnter"),
            atom("XdndPosition"),
            atom("XdndStatus"),
            atom("XdndLeave"),
            atom("XdndDrop"),
            atom("XdndFinished"),
            atom("XdndSelection"),
            atom("XdndActionCopy"),
        );
        let (uris, utf8) = (atom("text/uri-list"), atom("UTF8_STRING"));
        let win = conn.generate_id().unwrap();
        let aux = CreateWindowAux::new().background_pixel(0x0080_4040);
        conn.create_window(COPY_DEPTH_FROM_PARENT, win, root, 0, 0, 400, 300, 0, WindowClass::INPUT_OUTPUT, 0, &aux)
            .unwrap();
        conn.change_property8(PropMode::REPLACE, win, AtomEnum::WM_CLASS, AtomEnum::STRING, b"x\0Source\0").unwrap();
        conn.change_property8(PropMode::REPLACE, win, AtomEnum::WM_NAME, AtomEnum::STRING, b"source").unwrap();
        conn.map_window(win).unwrap();
        conn.flush().unwrap();
        let send = |to: u32, kind: Atom, data: [u32; 5]| {
            let ev = x11rb::protocol::xproto::ClientMessageEvent::new(32, to, kind, data);
            conn.send_event(false, to, EventMask::NO_EVENT, ev).unwrap();
            conn.flush().unwrap();
        };
        let (mut dragging, mut dropped, mut accepted) = (false, false, false);
        let mut target: Option<(u32, bool)> = None;
        let deadline = Instant::now() + Duration::from_secs(9);
        while Instant::now() < deadline {
            while let Some(event) = conn.poll_for_event().unwrap() {
                match event {
                    Event::ClientMessage(m) if m.type_ == status => accepted = m.data.as_data32()[1] & 1 == 1,
                    Event::ClientMessage(m) if m.type_ == finished => {
                        conn.sync().unwrap();
                        return Some(m.data.as_data32()[1] & 1 == 1);
                    }
                    Event::SelectionRequest(r) if r.selection == selection => {
                        let ok = r.target == uris || r.target == utf8;
                        if ok {
                            conn.change_property8(PropMode::REPLACE, r.requestor, r.property, r.target, URIS).unwrap();
                        }
                        let notify = SelectionNotifyEvent {
                            response_type: SELECTION_NOTIFY_EVENT,
                            sequence: 0,
                            time: r.time,
                            requestor: r.requestor,
                            selection: r.selection,
                            target: r.target,
                            property: if ok { r.property } else { x11rb::NONE },
                        };
                        conn.send_event(false, r.requestor, EventMask::NO_EVENT, notify).unwrap();
                        conn.flush().unwrap();
                    }
                    _ => {}
                }
            }
            if !dropped {
                let p = conn.query_pointer(root).unwrap().reply().unwrap();
                let held = u16::from(p.mask) & u16::from(KeyButMask::BUTTON1) != 0;
                if held && !dragging {
                    dragging = true;
                    conn.set_selection_owner(win, selection, x11rb::CURRENT_TIME).unwrap();
                    conn.flush().unwrap();
                } else if dragging && held {
                    let under = p.child;
                    if target.map(|(t, _)| t) != Some(under) {
                        if let Some((old, true)) = target {
                            send(old, leave, [win, 0, 0, 0, 0]);
                        }
                        accepted = false;
                        let takes = under != x11rb::NONE
                            && conn
                                .get_property(false, under, aware, AtomEnum::ATOM, 0, 1)
                                .unwrap()
                                .reply()
                                .unwrap()
                                .value_len
                                > 0;
                        if takes {
                            send(under, enter, [win, 5 << 24, uris, utf8, 0]);
                        }
                        target = Some((under, takes));
                    }
                    if let Some((t, true)) = target {
                        let at = ((p.root_x as u32) << 16) | (p.root_y as u16 as u32);
                        send(t, position, [win, 0, at, x11rb::CURRENT_TIME, copy]);
                    }
                } else if dragging && !held {
                    match target {
                        Some((t, true)) if accepted => {
                            send(t, drop, [win, 0, x11rb::CURRENT_TIME, 0, 0]);
                            dropped = true;
                        }
                        _ => return None,
                    }
                }
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        panic!("the drag never finished (dragging {dragging}, dropped {dropped}, target {target:?})");
    })
}

#[test]
fn an_x11_app_drags_a_file_onto_a_wayland_app() {
    use wayland_client::protocol::{wl_data_device_manager::WlDataDeviceManager, wl_seat::WlSeat};
    with_x11(|h| {
        set(&mut h.state, "windows.layout", json!("dwindle"));
        h.drive(|conn, ask| {
            let mut c = Client::connect(&conn);
            c.open_bar();
            let (window, _t) = open(&mut c);
            c.attach_buffer(&window, LEFT.2, LEFT.3);
            X::connect(&ask);
            let display = ask.run(|s| s.xwayland.display.unwrap());
            let source = x11_drag_source(display);
            opened(&ask, "Source");
            until(&ask, "they tile", |s| by_class(s, "Source").is_some_and(|w| rect(s, &w) == Some(RIGHT)));

            let (globals, mut q) =
                wayland_client::globals::registry_queue_init::<drop_target::DropTarget>(&conn).unwrap();
            let qh = q.handle();
            let seat: WlSeat = globals.bind(&qh, 1..=7, ()).unwrap();
            let manager: WlDataDeviceManager = globals.bind(&qh, 3..=3, ()).unwrap();
            let _device = manager.get_data_device(&seat, &qh, ());
            let mut d = drop_target::DropTarget::default();
            q.roundtrip(&mut d).unwrap();

            let time = Arc::new(std::sync::atomic::AtomicU32::new(0));
            let mv = |x: f64, y: f64| {
                let t = time.fetch_add(10, Ordering::Relaxed);
                ask.run(move |s| s.on_pointer_motion((x, y).into(), t));
            };
            let button = |pressed: bool| {
                let t = time.fetch_add(10, Ordering::Relaxed);
                let state = if pressed {
                    smithay::backend::input::ButtonState::Pressed
                } else {
                    smithay::backend::input::ButtonState::Released
                };
                ask.run(move |s| s.on_pointer_button(BTN_LEFT, state, t));
            };

            // Press on the X11 app (inside the 400x300 it drew), and drag
            // across to the middle of the Wayland window.
            let (x0, y0) = ((RIGHT.0 + 100) as f64, (RIGHT.1 + 100) as f64);
            let (x1, y1) = ((LEFT.0 + LEFT.2 / 2) as f64, (LEFT.1 + LEFT.3 / 2) as f64);
            mv(x0, y0);
            button(true);
            std::thread::sleep(Duration::from_millis(100));
            for i in 1..=20 {
                let f = f64::from(i) / 20.0;
                mv(x0 + (x1 - x0) * f, y0 + (y1 - y0) * f);
                std::thread::sleep(Duration::from_millis(30));
                q.roundtrip(&mut d).unwrap();
            }
            // A little more over it, for the X11 app to hear it's taken.
            for i in 0..6 {
                mv(x1 + f64::from(i), y1);
                std::thread::sleep(Duration::from_millis(30));
                q.roundtrip(&mut d).unwrap();
            }
            let offer = d.entered.clone().expect("the Wayland window was offered the drag");
            let mimes = d.offers.iter().find(|(o, _)| *o == offer).unwrap().1.clone();

            // Drop: the Wayland window reads the file list and finishes.
            button(false);
            let deadline = Instant::now() + Duration::from_secs(5);
            while !d.dropped {
                assert!(Instant::now() < deadline, "no drop on the Wayland window");
                q.roundtrip(&mut d).unwrap();
                std::thread::sleep(Duration::from_millis(10));
            }
            let (read, write) = UnixStream::pair().unwrap();
            read.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
            offer.receive("text/uri-list".into(), std::os::fd::AsFd::as_fd(&write));
            drop(write);
            q.roundtrip(&mut d).unwrap();
            let reader = std::thread::spawn(move || {
                let mut got = Vec::new();
                (&read).read_to_end(&mut got).unwrap();
                got
            });
            while !reader.is_finished() {
                q.roundtrip(&mut d).unwrap();
                std::thread::sleep(Duration::from_millis(10));
            }
            let got = reader.join().unwrap();
            offer.finish();
            q.roundtrip(&mut d).unwrap();
            let finished = source.join().unwrap();

            assert!(mimes.iter().any(|m| m == "text/uri-list"), "offered {mimes:?}");
            assert!(mimes.iter().any(|m| m == "text/plain;charset=utf-8"), "UTF8_STRING offered as text: {mimes:?}");
            assert_eq!(got, URIS, "the Wayland window got the file list");
            assert_eq!(finished, Some(true), "the X11 app heard its drop finished");
        });
    });
}

#[test]
fn an_x11_drag_dropped_on_the_desktop_is_no_drop() {
    with_x11(|h| {
        h.drive(|conn, ask| {
            let mut c = Client::connect(&conn);
            c.open_bar();
            X::connect(&ask);
            let display = ask.run(|s| s.xwayland.display.unwrap());
            let source = x11_drag_source(display);
            opened(&ask, "Source");
            let (x0, y0) = ask.run(|s| {
                let (x, y, _, _) = rect(s, &by_class(s, "Source").unwrap()).unwrap();
                ((x + 100) as f64, (y + 100) as f64)
            });
            let time = Arc::new(std::sync::atomic::AtomicU32::new(0));
            let input = |motion: Option<(f64, f64)>, pressed: Option<bool>| {
                let t = time.fetch_add(10, Ordering::Relaxed);
                ask.run(move |s| {
                    if let Some(p) = motion {
                        s.on_pointer_motion(p.into(), t);
                    }
                    if let Some(p) = pressed {
                        let state = if p {
                            smithay::backend::input::ButtonState::Pressed
                        } else {
                            smithay::backend::input::ButtonState::Released
                        };
                        s.on_pointer_button(BTN_LEFT, state, t);
                    }
                })
            };
            input(Some((x0, y0)), Some(true));
            std::thread::sleep(Duration::from_millis(100));
            // Off to empty desktop, far from its window.
            for i in 1..=10 {
                input(Some((x0 + 80.0 * f64::from(i), y0 + 40.0 * f64::from(i))), None);
                std::thread::sleep(Duration::from_millis(30));
            }
            input(None, Some(false));
            assert_eq!(source.join().unwrap(), None, "nothing took it, so no drop");
            until(&ask, "nothing left of the drag", |s| s.xwayland.dnd.idle());
        });
    });
}
