//! Screen locking with a real ext-session-lock-v1 client, the way QuickShell's
//! WlSessionLock uses it.

mod common;

use common::*;
use smithay::{backend::input::KeyState, input::keyboard::Keycode};
use wayland_client::{
    protocol::{
        wl_keyboard::{self, WlKeyboard},
        wl_output::WlOutput,
        wl_pointer::{self, WlPointer},
        wl_registry::WlRegistry,
        wl_seat::{self, WlSeat},
        wl_surface::WlSurface,
    },
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
};
use wayland_protocols::ext::session_lock::v1::client::{
    ext_session_lock_manager_v1::ExtSessionLockManagerV1,
    ext_session_lock_surface_v1::{self, ExtSessionLockSurfaceV1},
    ext_session_lock_v1::{self, ExtSessionLockV1},
};
use whiterose_wm::WhiteRose;

const SUPER: u32 = 125 + 8;
const KEY_2: u32 = 3 + 8;

#[derive(Default)]
struct L {
    locked: bool,
    finished: bool,
    configure: Option<(u32, u32)>,
    pointer_on: Option<WlSurface>,
    keyboard_on: Option<WlSurface>,
}

impl Dispatch<WlRegistry, wayland_client::globals::GlobalListContents> for L {
    fn event(
        _: &mut Self,
        _: &WlRegistry,
        _: wayland_client::protocol::wl_registry::Event,
        _: &wayland_client::globals::GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<WlSeat, ()> for L {
    fn event(_: &mut Self, _: &WlSeat, _: wl_seat::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<WlOutput, ()> for L {
    fn event(
        _: &mut Self,
        _: &WlOutput,
        _: wayland_client::protocol::wl_output::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<ExtSessionLockManagerV1, ()> for L {
    fn event(
        _: &mut Self,
        _: &ExtSessionLockManagerV1,
        _: <ExtSessionLockManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<ExtSessionLockV1, ()> for L {
    fn event(
        l: &mut Self,
        _: &ExtSessionLockV1,
        e: ext_session_lock_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match e {
            ext_session_lock_v1::Event::Locked => l.locked = true,
            ext_session_lock_v1::Event::Finished => l.finished = true,
            _ => {}
        }
    }
}
impl Dispatch<ExtSessionLockSurfaceV1, ()> for L {
    fn event(
        l: &mut Self,
        s: &ExtSessionLockSurfaceV1,
        e: ext_session_lock_surface_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_session_lock_surface_v1::Event::Configure { serial, width, height } = e {
            s.ack_configure(serial);
            l.configure = Some((width, height));
        }
    }
}
impl Dispatch<WlPointer, ()> for L {
    fn event(l: &mut Self, _: &WlPointer, e: wl_pointer::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match e {
            wl_pointer::Event::Enter { surface, .. } => l.pointer_on = Some(surface),
            wl_pointer::Event::Leave { .. } => l.pointer_on = None,
            _ => {}
        }
    }
}
impl Dispatch<WlKeyboard, ()> for L {
    fn event(l: &mut Self, _: &WlKeyboard, e: wl_keyboard::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {
        match e {
            wl_keyboard::Event::Enter { surface, .. } => l.keyboard_on = Some(surface),
            wl_keyboard::Event::Leave { .. } => l.keyboard_on = None,
            _ => {}
        }
    }
}

struct Seat {
    q: EventQueue<L>,
    l: L,
    output: WlOutput,
    manager: ExtSessionLockManagerV1,
    _pointer: WlPointer,
    _keyboard: WlKeyboard,
}

impl Seat {
    fn bind(conn: &Connection) -> Seat {
        let (globals, mut q) = wayland_client::globals::registry_queue_init::<L>(conn).unwrap();
        let qh = q.handle();
        let seat: WlSeat = globals.bind(&qh, 1..=7, ()).unwrap();
        Seat {
            output: globals.bind(&qh, 1..=4, ()).unwrap(),
            manager: globals.bind(&qh, 1..=1, ()).expect("ext_session_lock_manager_v1"),
            _pointer: seat.get_pointer(&qh, ()),
            _keyboard: seat.get_keyboard(&qh, ()),
            l: L::default(),
            q: {
                let mut l = L::default();
                q.roundtrip(&mut l).unwrap();
                q
            },
        }
    }
    fn until(&mut self, what: &str, done: impl Fn(&L) -> bool) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
        self.q.roundtrip(&mut self.l).unwrap();
        while !done(&self.l) {
            assert!(std::time::Instant::now() < deadline, "timed out waiting for {what}");
            self.q.blocking_dispatch(&mut self.l).unwrap();
        }
    }
}

fn key(state: &mut WhiteRose, code: u32, down: bool) {
    state.on_key(Keycode::new(code), if down { KeyState::Pressed } else { KeyState::Released }, 0);
}

#[derive(Debug, Default)]
struct Seen {
    locked_before_frame: bool,
    confirmed: bool,
    configure: Option<(u32, u32)>,
    pointer_on_lock: bool,
    keyboard_on_lock: bool,
    workspace_after_shortcut: usize,
    screenshot_pending: bool,
    window_got_focus_while_locked: bool,
    preview_events: Vec<String>,
    locked_after_unlock: bool,
    keyboard_back_on_window: bool,
}

#[test]
fn lock_shows_only_the_lock_screen_and_unlock_restores_the_desktop() {
    let mut h = Headless::new(true);
    let seen = h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        let (window, _toplevel) = c.open_window();
        c.attach_buffer(&window, 800, 600);
        let mut s = Seat::bind(&conn);
        let qh = s.q.handle();

        let lock = s.manager.lock(&qh, ());
        s.q.roundtrip(&mut s.l).unwrap();
        let mut seen = Seen { locked_before_frame: s.l.locked, ..Default::default() };
        assert!(ask.run(|st| st.is_locked()), "locked as soon as asked");

        // Confirmed only once a frame without the desktop was shown.
        ask.run(|st| st.frame_presented());
        s.until("locked", |l| l.locked);
        seen.confirmed = true;

        // The lock surface covers the output and gets all input.
        let surface = c.compositor.create_surface(&c.queue.handle(), ());
        let _lock_surface = lock.get_lock_surface(&surface, &s.output, &qh, ());
        s.until("configure", |l| l.configure.is_some());
        seen.configure = s.l.configure;
        let (w, h) = s.l.configure.unwrap();
        c.attach_buffer(&surface, w as i32, h as i32);
        ask.run(|st| st.on_pointer_motion((400.0, 300.0).into(), 1));
        s.until("pointer enter", |l| l.pointer_on.is_some());
        seen.pointer_on_lock = s.l.pointer_on.as_ref() == Some(&surface);
        seen.keyboard_on_lock = s.l.keyboard_on.as_ref() == Some(&surface);

        // No shortcuts (SUPER+2 would switch workspace), no screenshots, and
        // clicking where the window is doesn't focus it.
        seen.workspace_after_shortcut = ask.run(|st| {
            key(st, SUPER, true);
            key(st, KEY_2, true);
            key(st, KEY_2, false);
            key(st, SUPER, false);
            st.workspaces.active()
        });
        seen.screenshot_pending = ask.run(|st| {
            st.request_screenshot();
            st.screenshot_pending
        });
        seen.window_got_focus_while_locked = ask.run(|st| {
            let w = st.space.elements().next().cloned();
            st.focus_window(w);
            st.keyboard_focus().is_some_and(|f| st.window_for_surface(&f).is_some())
        });

        // Window previews (dock, switcher) are refused: no pixels of the
        // desktop behind the lock can be read.
        {
            let (globals, _) = wayland_client::globals::registry_queue_init::<App>(&conn).unwrap();
            let cq = c.queue.handle();
            let _m: ZwlrForeignToplevelManagerV1 = globals.bind(&cq, 1..=3, ()).unwrap();
            let export: hyprland_toplevel_export_manager_v1::HyprlandToplevelExportManagerV1 =
                globals.bind(&cq, 2..=2, ()).unwrap();
            c.queue.roundtrip(&mut c.app).unwrap();
            let handle = c.app.handles.first().expect("a handle for the window").0.clone();
            let _frame = export.capture_toplevel_with_wlr_toplevel_handle(0, &handle, &cq, ());
            c.until(|a| !a.export_events.is_empty());
            seen.preview_events = c.app.export_events.clone();
        }

        lock.unlock_and_destroy();
        s.q.roundtrip(&mut s.l).unwrap();
        seen.locked_after_unlock = ask.run(|st| st.is_locked());
        s.until("keyboard back on the window", |l| l.keyboard_on.is_some());
        seen.keyboard_back_on_window = s.l.keyboard_on.as_ref() == Some(&window);
        seen
    });

    assert!(!seen.locked_before_frame, "`locked` must wait until the desktop is off screen");
    assert!(seen.confirmed);
    assert_eq!(seen.configure, Some((W as u32, H as u32)), "lock surface fills the output");
    assert!(seen.pointer_on_lock, "pointer goes to the lock surface, not the window under it");
    assert!(seen.keyboard_on_lock, "keyboard goes to the lock surface");
    assert_eq!(seen.workspace_after_shortcut, 0, "shortcuts don't run while locked");
    assert!(!seen.screenshot_pending, "no screenshots of the lock screen");
    assert!(!seen.window_got_focus_while_locked, "windows can't take the keyboard while locked");
    assert_eq!(seen.preview_events, ["failed"], "window previews are refused while locked");
    assert!(!seen.locked_after_unlock);
    assert!(seen.keyboard_back_on_window, "unlocking gives the keyboard back to the window");
}

#[test]
fn a_crashed_lock_screen_leaves_the_session_locked_until_a_new_one_unlocks() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut s = Seat::bind(&conn);
        let qh = s.q.handle();
        let _lock = s.manager.lock(&qh, ());
        s.q.roundtrip(&mut s.l).unwrap();
        ask.run(|st| st.frame_presented());
        s.until("locked", |l| l.locked);
        // The client now exits without unlocking (crash).
    });
    for _ in 0..10 {
        h.dispatch();
    }
    assert!(h.state.is_locked(), "a dead lock client must not unlock the session");

    // The restarted shell locks again, takes over, and can unlock.
    h.drive(|conn, ask| {
        let mut s = Seat::bind(&conn);
        let qh = s.q.handle();
        let lock = s.manager.lock(&qh, ());
        s.q.roundtrip(&mut s.l).unwrap();
        ask.run(|st| st.frame_presented());
        s.until("locked by the new client", |l| l.locked || l.finished);
        assert!(s.l.locked && !s.l.finished, "the new lock client is accepted");
        lock.unlock_and_destroy();
        s.q.roundtrip(&mut s.l).unwrap();
        assert!(!ask.run(|st| st.is_locked()));
    });
}

#[test]
fn a_lock_cancelled_before_it_took_effect_does_not_leave_the_session_locked() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut s = Seat::bind(&conn);
        let qh = s.q.handle();
        let lock = s.manager.lock(&qh, ());
        s.q.roundtrip(&mut s.l).unwrap();
        assert!(ask.run(|st| st.is_locked()));
        // No frame shown yet, so no `locked`: the client may still give up
        // (QuickShell does this if told to unlock before the lock is secure).
        lock.destroy();
        s.q.roundtrip(&mut s.l).unwrap();
        assert!(!ask.run(|st| st.is_locked()), "a cancelled lock must not keep the screen locked");
    });
}

#[test]
fn only_the_lock_in_charge_can_unlock() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        // An old lock screen that's still connected, and a newer one that took over.
        let mut old = Seat::bind(&conn);
        let old_lock = old.manager.lock(&old.q.handle(), ());
        old.q.roundtrip(&mut old.l).unwrap();
        ask.run(|st| st.frame_presented());
        old.until("old locked", |l| l.locked);

        let mut new = Seat::bind(&conn);
        let new_lock = new.manager.lock(&new.q.handle(), ());
        new.q.roundtrip(&mut new.l).unwrap();
        ask.run(|st| st.frame_presented());
        new.until("new locked", |l| l.locked);

        old_lock.unlock_and_destroy();
        old.q.roundtrip(&mut old.l).unwrap();
        assert!(ask.run(|st| st.is_locked()), "a replaced lock can't unlock the session");

        new_lock.unlock_and_destroy();
        new.q.roundtrip(&mut new.l).unwrap();
        assert!(!ask.run(|st| st.is_locked()));
    });
}

/// Regression: QuickShell crashed (disconnected by the compositor) on every
/// unlock. After unlocking it destroys the lock surface object, then Qt hides
/// the window by committing a NULL buffer to the plain wl_surface; Smithay
/// still ran its lock surface checks on that commit and killed the client.
#[test]
fn hiding_a_former_lock_surface_after_unlock_does_not_kill_the_client() {
    let mut h = Headless::new(true);
    h.drive(|conn, ask| {
        let mut c = Client::connect(&conn);
        let mut s = Seat::bind(&conn);
        let qh = s.q.handle();
        let lock = s.manager.lock(&qh, ());
        s.q.roundtrip(&mut s.l).unwrap();
        ask.run(|st| st.frame_presented());
        s.until("locked", |l| l.locked);
        let surface = c.compositor.create_surface(&c.queue.handle(), ());
        let lock_surface = lock.get_lock_surface(&surface, &s.output, &qh, ());
        s.until("configure", |l| l.configure.is_some());
        let (w, h) = s.l.configure.unwrap();
        c.attach_buffer(&surface, w as i32, h as i32);

        // What QuickShell does on unlock, in order.
        lock.unlock_and_destroy();
        lock_surface.destroy();
        surface.attach(None, 0, 0);
        surface.commit();
        let alive = c.queue.roundtrip(&mut c.app).is_ok() && s.q.roundtrip(&mut s.l).is_ok();
        assert!(alive, "the compositor disconnected the client: {:?}", conn.protocol_error());
        assert!(!ask.run(|st| st.is_locked()));
    });
}
