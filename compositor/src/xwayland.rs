//! XWayland: X11 apps (Spotify, Steam, older tools) in WhiteRose
//! (docs/plans/xwayland.md). Xwayland starts with the compositor; DISPLAY is
//! set before the daemon and shell start, so everything launched inherits it.
//!
//! A managed X11 window is a window like any other: it opens as a Wayland
//! one does (under the pointer, on this workspace, focused) and its first
//! commit applies the rules and tiles (`window_committed`). Override-
//! redirect ones (menus, tooltips) are drawn where they ask, on top, outside
//! the workspaces, and never take focus.
//!
//! X11 is told a window's place and size together, so after each batch of
//! events `sync_x11` sends any X11 window whose rectangle (where the
//! compositor has it, the size it's told) differs from what X has.

use smithay::{
    backend::renderer::utils::with_renderer_surface_state,
    desktop::Window,
    input::pointer::GrabStartData as PointerGrabStartData,
    reexports::wayland_server::protocol::wl_surface::WlSurface,
    reexports::{
        calloop::{
            self,
            generic::Generic,
            timer::{TimeoutAction, Timer},
            Interest, Mode, PostAction, RegistrationToken,
        },
        wayland_protocols::xdg::shell::server::xdg_toplevel::State as ToplevelState,
    },
    utils::{Logical, Rectangle, SERIAL_COUNTER},
    wayland::{
        selection::SelectionTarget,
        xwayland_shell::{XWaylandShellHandler, XWaylandShellState},
    },
    xwayland::{
        xwm::{Reorder, ResizeEdge as X11ResizeEdge, WmWindowProperty, XwmId},
        X11Surface, X11Wm, XWayland, XWaylandEvent, XWaylandSockets, XwmHandler,
    },
};

use std::{os::fd::OwnedFd, time::Duration};

use smithay::wayland::selection::data_device::{
    clear_data_device_selection, current_data_device_selection_userdata, set_data_device_selection,
};

use crate::{clipboard_capture::Offer, state::WhiteRose, win::WinExt};

/// An override-redirect window (a menu, a tooltip): not managed.
#[derive(Debug)]
pub struct Unmanaged;

pub fn unmanaged(w: &Window) -> bool {
    w.user_data().get::<Unmanaged>().is_some()
}

#[derive(Debug)]
pub struct XState {
    pub shell: XWaylandShellState,
    pub wm: Option<X11Wm>,
    /// The display it serves (":N"), bound at startup.
    pub display: Option<u32>,
    sockets: Option<XWaylandSockets>,
    /// Watching the sockets for the first X11 app (while Xwayland isn't running).
    watches: Vec<RegistrationToken>,
    /// The running Xwayland's startup source.
    server: Option<RegistrationToken>,
    /// Starts that failed in a row (it stops trying after a few).
    failures: u32,
    /// How often (seconds) a running Xwayland is asked whether any X11 app
    /// is still connected; it stops after two answers of "none" in a row.
    pub linger: u32,
    /// "None connected" answers in a row.
    idle: u32,
    /// The idle check's timer, while Xwayland runs.
    check: Option<RegistrationToken>,
    /// Where idle checks (on a worker thread) send their answer.
    checked: Option<calloop::channel::Sender<bool>>,
    /// The Wayland clipboard's types, announced to an Xwayland that starts
    /// after the copy (None: nothing copied, or an X11 app's copy).
    pub clipboard: Option<Vec<String>>,
}

impl XState {
    pub fn new(dh: &smithay::reexports::wayland_server::DisplayHandle) -> Self {
        XState {
            shell: XWaylandShellState::new::<WhiteRose>(dh),
            wm: None,
            display: None,
            sockets: None,
            watches: Vec::new(),
            server: None,
            failures: 0,
            linger: 10,
            idle: 0,
            check: None,
            checked: None,
            clipboard: None,
        }
    }

    /// Whether Xwayland is running now.
    pub fn running(&self) -> bool {
        self.server.is_some()
    }
}

/// Starts after this many failures in a row: then X11 apps just don't open.
const MAX_FAILURES: u32 = 3;

impl WhiteRose {
    /// Takes an X display and sets DISPLAY for what the compositor starts
    /// from now on; Xwayland itself starts when the first X11 app connects,
    /// and exits a while after the last one goes (docs/plans/xwayland.md).
    /// Without an Xwayland installed, nothing: X11 apps just don't open.
    pub fn listen_xwayland(&mut self) {
        if !in_path("Xwayland") {
            tracing::info!("no Xwayland installed: X11 apps won't open");
            return;
        }
        let sockets = match XWaylandSockets::bind(None, true) {
            Ok(s) => s,
            Err(err) => {
                tracing::warn!(%err, "no free X display: X11 apps won't open");
                return;
            }
        };
        let n = sockets.display_number();
        self.xwayland.display = Some(n);
        self.xwayland.sockets = Some(sockets);
        // (Before the daemon and shell start: they and what they launch inherit it.)
        std::env::set_var("DISPLAY", format!(":{n}"));
        tracing::info!(display = n, "X11 apps can connect (Xwayland starts with the first)");
        let (tx, rx) = calloop::channel::channel::<bool>();
        let _ = self.loop_handle.insert_source(rx, |event, _, state| {
            if let calloop::channel::Event::Msg(idle) = event {
                state.x11_checked(idle);
            }
        });
        self.xwayland.checked = Some(tx);
        self.watch_x11_sockets();
    }

    /// Waits for an X11 app to connect, to start Xwayland for it.
    fn watch_x11_sockets(&mut self) {
        let Some(sockets) = self.xwayland.sockets.as_ref() else { return };
        let fds: Vec<_> = sockets.listeners().iter().filter_map(|l| l.try_clone().ok()).collect();
        for fd in fds {
            let source = Generic::new(fd, Interest::READ, Mode::Level);
            match self.loop_handle.insert_source(source, |_, _, state| {
                state.start_xwayland();
                Ok(PostAction::Continue)
            }) {
                Ok(token) => self.xwayland.watches.push(token),
                Err(err) => tracing::warn!(%err, "couldn't watch the X11 socket"),
            }
        }
    }

    /// An X11 app is connecting: starts Xwayland, which takes it.
    fn start_xwayland(&mut self) {
        for token in std::mem::take(&mut self.xwayland.watches) {
            self.loop_handle.remove(token);
        }
        let Some(sockets) = self.xwayland.sockets.as_ref() else { return };
        let spawned = XWayland::spawn_with_sockets(
            &self.display_handle,
            sockets,
            std::iter::empty::<(String, String)>(),
            std::process::Stdio::null(),
            std::process::Stdio::null(),
            |_| {},
        );
        let (xwayland, client) = match spawned {
            Ok(x) => x,
            Err(err) => {
                tracing::warn!(%err, "XWayland didn't start");
                self.xwayland_failed();
                return;
            }
        };
        let inserted = self.loop_handle.insert_source(xwayland, move |event, _, state| match event {
            XWaylandEvent::Ready { x11_socket, display_number } => {
                match X11Wm::start_wm(state.loop_handle.clone(), x11_socket, client.clone()) {
                    Ok(mut wm) => {
                        tracing::info!(display = display_number, "XWayland is running");
                        state.xwayland.failures = 0;
                        // What Wayland apps copied before it started.
                        let _ = wm.new_selection(SelectionTarget::Clipboard, state.xwayland.clipboard.clone());
                        state.xwayland.wm = Some(wm);
                        state.start_x11_checks();
                    }
                    Err(err) => {
                        tracing::warn!(%err, "the X11 window manager didn't start");
                        state.stop_xwayland();
                        state.xwayland_failed();
                    }
                }
            }
            XWaylandEvent::Error => {
                tracing::warn!("XWayland didn't start");
                state.stop_xwayland();
                state.xwayland_failed();
            }
        });
        match inserted {
            Ok(token) => self.xwayland.server = Some(token),
            Err(err) => tracing::warn!(%err, "XWayland's events couldn't be watched"),
        }
    }

    /// A start failed: tries again for the next X11 app, a few times.
    fn xwayland_failed(&mut self) {
        self.xwayland.failures += 1;
        if self.xwayland.failures >= MAX_FAILURES {
            tracing::warn!("XWayland keeps failing: X11 apps won't open until WhiteRose restarts");
            return;
        }
        self.watch_x11_sockets();
    }

    /// Every `linger` seconds while Xwayland runs, asks it (on a worker
    /// thread: X may be waiting on us) whether any X11 app is connected.
    fn start_x11_checks(&mut self) {
        let every = Duration::from_secs(self.xwayland.linger.max(1).into());
        let timer = Timer::from_duration(every);
        let token = self.loop_handle.insert_source(timer, move |_, _, state| {
            let (Some(n), Some(tx)) = (state.xwayland.display, state.xwayland.checked.clone()) else {
                return TimeoutAction::Drop;
            };
            std::thread::spawn(move || {
                if let Some(idle) = x11_idle(n) {
                    let _ = tx.send(idle);
                }
            });
            TimeoutAction::ToDuration(every)
        });
        self.xwayland.check = token.ok();
    }

    /// An idle check's answer: twice "no X11 app" in a row stops Xwayland,
    /// and the next X11 app starts it again.
    fn x11_checked(&mut self, idle: bool) {
        if !self.xwayland.running() {
            return;
        }
        self.xwayland.idle = if idle { self.xwayland.idle + 1 } else { 0 };
        if self.xwayland.idle >= 2 {
            tracing::info!("no X11 apps left: XWayland stops until the next one");
            self.stop_xwayland();
            self.watch_x11_sockets();
        }
    }

    /// Xwayland is gone (stopped when no X11 app was left, or failed): its
    /// windows go, its copy leaves the clipboard.
    fn stop_xwayland(&mut self) {
        self.xwayland.wm = None;
        self.xwayland.idle = 0;
        if let Some(token) = self.xwayland.check.take() {
            self.loop_handle.remove(token);
        }
        if let Some(token) = self.xwayland.server.take() {
            self.loop_handle.remove(token);
        }
        let windows: Vec<Window> = self
            .space
            .elements()
            .chain(self.hidden_at.iter().map(|(w, _)| w))
            .filter(|w| crate::win::is_x11(w))
            .cloned()
            .collect();
        for w in windows {
            if unmanaged(&w) {
                self.space.unmap_elem(&w);
            } else {
                self.forget_window(&w);
            }
        }
        if matches!(current_data_device_selection_userdata(&self.seat).as_deref(), Some(Offer::X11)) {
            clear_data_device_selection(&self.display_handle, &self.seat);
        }
        self.schedule_redraw();
    }

    /// The managed X11 window of `x`, if it's on screen or hidden.
    fn x11_window(&self, x: &X11Surface) -> Option<Window> {
        self.space.elements().chain(self.hidden_at.iter().map(|(w, _)| w)).find(|w| w.x11_surface() == Some(x)).cloned()
    }

    /// Tells X11 windows where they are and their size, and their states,
    /// where that's changed (after each batch of events).
    pub fn sync_x11(&mut self) {
        if self.xwayland.wm.is_none() {
            return;
        }
        let windows: Vec<Window> =
            self.space.elements().filter(|w| w.x11_surface().is_some() && !unmanaged(w)).cloned().collect();
        for w in windows {
            let Some(x) = w.x11_surface() else { continue };
            let Some(loc) = self.space.element_location(&w) else { continue };
            let next = crate::win::take_x11_next(&w);
            let size = crate::win::x11_size(&w).filter(|s| s.w > 0 && s.h > 0).unwrap_or(x.geometry().size);
            let rect = Rectangle::new(loc, size);
            if let Some(n) = &next {
                let _ = x.set_maximized(n.contains(ToplevelState::Maximized));
                let _ = x.set_fullscreen(n.contains(ToplevelState::Fullscreen));
            }
            if next.is_some() || x.geometry() != rect {
                let _ = x.configure(rect);
            }
        }
    }

    /// The X11 half of focusing `w` (input focus, and on top in X's stack).
    pub(crate) fn x11_focus(&mut self, w: &Window) {
        let Some(x) = w.x11_surface().cloned() else { return };
        x.give_focus();
        if let Some(wm) = self.xwayland.wm.as_mut() {
            let _ = wm.raise_window(&x);
        }
    }

    /// Where the pointer is, for a move or resize an X11 app asked for.
    fn x11_grab_start(&self, button: u32) -> Option<PointerGrabStartData<WhiteRose>> {
        let pointer = self.seat.get_pointer()?;
        Some(PointerGrabStartData { focus: None, button, location: pointer.current_location() })
    }
}

impl XWaylandShellHandler for WhiteRose {
    fn xwayland_shell_state(&mut self) -> &mut XWaylandShellState {
        &mut self.xwayland.shell
    }

    /// Its wl_surface is known. Usually that's during the surface's commit,
    /// which then does what a commit does; when X says so only after the
    /// surface has drawn, a window that doesn't draw again (xmessage) would
    /// never get its rules or tile, so that's done here.
    fn surface_associated(&mut self, _: XwmId, surface: WlSurface, x: X11Surface) {
        let drawn = with_renderer_surface_state(&surface, |s| s.buffer().is_some()).unwrap_or(false);
        if let Some(w) = self.x11_window(&x).filter(|_| drawn) {
            self.window_commit(&w);
            self.schedule_redraw();
        }
    }
}
smithay::delegate_xwayland_shell!(WhiteRose);

impl XwmHandler for WhiteRose {
    /// Xwayland went away on its own (it crashed): the next X11 app starts
    /// it again. (One we stopped is already forgotten.)
    fn disconnected(&mut self, xwm: XwmId) {
        if self.xwayland.wm.as_ref().map(|w| w.id()) != Some(xwm) {
            return;
        }
        tracing::warn!("XWayland exited: it starts again with the next X11 app");
        self.stop_xwayland();
        self.watch_x11_sockets();
    }

    fn xwm_state(&mut self, _: XwmId) -> &mut X11Wm {
        self.xwayland.wm.as_mut().expect("an X11 window manager is running")
    }

    fn new_window(&mut self, _: XwmId, _: X11Surface) {}
    fn new_override_redirect_window(&mut self, _: XwmId, _: X11Surface) {}

    fn map_window_request(&mut self, _: XwmId, x: X11Surface) {
        if x.set_mapped(true).is_err() {
            return;
        }
        let window = Window::new_x11_window(x.clone());
        // As a Wayland window opens: under the pointer, cascaded, on this
        // workspace (its first commit applies the rules and tiles it).
        let output = self.pointer_output();
        let n = self.workspaces.active_windows().len() as i32;
        let area = output.map(|o| self.usable_area_on(&o)).unwrap_or_default().loc;
        let loc = (area.x + 48 + 32 * (n % 8), area.y + 48 + 32 * (n % 8));
        self.space.map_element(window.clone(), loc, false);
        let _ = x.configure(Rectangle::new(loc.into(), x.geometry().size));
        self.assign_window_id(&window);
        self.workspaces.add(window.clone());
        tracing::info!(class = x.class(), workspace = self.workspaces.active() + 1, "X11 window opened");
        self.focus_window(Some(window));
    }

    fn mapped_override_redirect_window(&mut self, _: XwmId, x: X11Surface) {
        let geo = x.geometry();
        let window = Window::new_x11_window(x);
        window.user_data().insert_if_missing(|| Unmanaged);
        self.space.map_element(window, geo.loc, true);
        self.schedule_redraw();
    }

    fn unmapped_window(&mut self, _: XwmId, x: X11Surface) {
        let found = self.space.elements().find(|w| w.x11_surface() == Some(&x)).cloned();
        if let Some(w) = found {
            if unmanaged(&w) {
                self.space.unmap_elem(&w);
                self.schedule_redraw();
                return;
            }
        }
        if let Some(w) = self.x11_window(&x) {
            self.forget_window(&w);
            tracing::info!("X11 window closed");
        }
        if !x.is_override_redirect() {
            let _ = x.set_mapped(false);
        }
    }

    fn destroyed_window(&mut self, xwm: XwmId, x: X11Surface) {
        self.unmapped_window(xwm, x);
    }

    fn configure_request(
        &mut self,
        _: XwmId,
        x: X11Surface,
        req_x: Option<i32>,
        req_y: Option<i32>,
        w: Option<u32>,
        h: Option<u32>,
        _: Option<Reorder>,
    ) {
        let mut geo = x.geometry();
        let Some(window) = self.x11_window(&x) else {
            // Not shown yet: it may choose.
            if let Some(v) = req_x {
                geo.loc.x = v
            }
            if let Some(v) = req_y {
                geo.loc.y = v
            }
            if let Some(v) = w {
                geo.size.w = v as i32
            }
            if let Some(v) = h {
                geo.size.h = v as i32
            }
            let _ = x.configure(geo);
            return;
        };
        // Tiled, snapped, maximized or fullscreen: where the compositor put
        // it. Floating: the size it asks for, where it is.
        let placed =
            self.is_tiled(&window) || self.snap_of(&window).is_some() || Self::maximized_or_fullscreen(&window);
        if !placed {
            if let Some(v) = w {
                geo.size.w = v as i32
            }
            if let Some(v) = h {
                geo.size.h = v as i32
            }
            window.pend(|p| p.size = Some(geo.size));
        }
        if let Some(loc) = self.space.element_location(&window) {
            geo.loc = loc;
        }
        let _ = x.configure(geo);
    }

    fn configure_notify(&mut self, _: XwmId, x: X11Surface, geometry: Rectangle<i32, Logical>, _: Option<u32>) {
        // A menu moves itself.
        let found = self.space.elements().find(|w| w.x11_surface() == Some(&x) && unmanaged(w)).cloned();
        if let Some(w) = found {
            self.space.map_element(w, geometry.loc, true);
            self.schedule_redraw();
        }
    }

    fn maximize_request(&mut self, _: XwmId, x: X11Surface) {
        if let Some(w) = self.x11_window(&x) {
            self.maximize(&w);
        }
    }
    fn unmaximize_request(&mut self, _: XwmId, x: X11Surface) {
        if let Some(w) = self.x11_window(&x) {
            self.unmaximize(&w);
        }
    }
    fn fullscreen_request(&mut self, _: XwmId, x: X11Surface) {
        if let Some(w) = self.x11_window(&x) {
            self.fullscreen(&w);
        }
    }
    fn unfullscreen_request(&mut self, _: XwmId, x: X11Surface) {
        if let Some(w) = self.x11_window(&x) {
            self.unfullscreen(&w);
        }
    }
    fn minimize_request(&mut self, _: XwmId, x: X11Surface) {
        if let Some(w) = self.x11_window(&x) {
            self.minimize(&w);
        }
    }

    fn unminimize_request(&mut self, _: XwmId, x: X11Surface) {
        if let Some(w) = self.x11_window(&x) {
            self.activate_window(&w);
        }
    }

    fn property_notify(&mut self, _: XwmId, x: X11Surface, property: WmWindowProperty) {
        // A new title or class: the bar, dock, switchers and tab strips show them.
        if matches!(property, WmWindowProperty::Title | WmWindowProperty::Class) && self.x11_window(&x).is_some() {
            self.ipc_sync();
            if !self.tiling.strips.is_empty() {
                self.schedule_redraw();
            }
        }
    }

    fn move_request(&mut self, _: XwmId, x: X11Surface, button: u32) {
        let (Some(w), Some(start)) = (self.x11_window(&x), self.x11_grab_start(button)) else { return };
        self.start_move(w, start, SERIAL_COUNTER.next_serial());
    }

    fn resize_request(&mut self, _: XwmId, x: X11Surface, button: u32, edge: X11ResizeEdge) {
        let (Some(w), Some(start)) = (self.x11_window(&x), self.x11_grab_start(button)) else { return };
        use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::ResizeEdge as E;
        let edges = match edge {
            X11ResizeEdge::Top => E::Top,
            X11ResizeEdge::Bottom => E::Bottom,
            X11ResizeEdge::Left => E::Left,
            X11ResizeEdge::Right => E::Right,
            X11ResizeEdge::TopLeft => E::TopLeft,
            X11ResizeEdge::TopRight => E::TopRight,
            X11ResizeEdge::BottomLeft => E::BottomLeft,
            X11ResizeEdge::BottomRight => E::BottomRight,
        };
        self.start_resize(w, edges.into(), start, SERIAL_COUNTER.next_serial());
    }

    // ---- copy and paste between X11 and Wayland apps ----
    // The X11 clipboard and the Wayland one are kept as one: an X11 copy is
    // offered to Wayland apps (and recorded in the history), a Wayland copy
    // (or the compositor's own) is offered to X11 apps. The middle-click
    // primary selection stays among X11 apps: Wayland has none here.

    /// An X11 app may read the clipboard only while it has the keyboard, as
    /// a Wayland app can only paste then (no snooping from the background).
    fn allow_selection_access(&mut self, _: XwmId, selection: SelectionTarget) -> bool {
        selection == SelectionTarget::Clipboard && self.focused_window().is_some_and(|w| crate::win::is_x11(&w))
    }

    /// An X11 app copied.
    fn new_selection(&mut self, _: XwmId, selection: SelectionTarget, mime_types: Vec<String>) {
        if selection != SelectionTarget::Clipboard {
            return;
        }
        self.xwayland.clipboard = None;
        set_data_device_selection(&self.display_handle, &self.seat, mime_types.clone(), Offer::X11);
        self.capture_clipboard(mime_types);
    }

    /// The X11 app whose copy was on the clipboard let it go (it quit).
    fn cleared_selection(&mut self, _: XwmId, selection: SelectionTarget) {
        let ours = matches!(current_data_device_selection_userdata(&self.seat).as_deref(), Some(Offer::X11));
        if selection == SelectionTarget::Clipboard && ours {
            clear_data_device_selection(&self.display_handle, &self.seat);
        }
    }

    /// An X11 app pastes what a Wayland app (or the compositor) copied.
    fn send_selection(&mut self, _: XwmId, selection: SelectionTarget, mime_type: String, fd: OwnedFd) {
        let x11 = matches!(current_data_device_selection_userdata(&self.seat).as_deref(), Some(Offer::X11));
        if selection == SelectionTarget::Clipboard && !x11 {
            self.read_clipboard(mime_type, fd);
        }
    }
}

/// Whether `program` is on PATH.
fn in_path(program: &str) -> bool {
    std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(program).is_file()))
}

/// Whether no X11 app is connected to display `n`: only the X server
/// itself, our window manager and this question are. None if X didn't
/// answer.
fn x11_idle(n: u32) -> Option<bool> {
    use x11rb::{connection::Connection as _, protocol::res::ConnectionExt as _};
    let (conn, _) = x11rb::connect(Some(&format!(":{n}"))).ok()?;
    let me = conn.setup().resource_id_base;
    let clients = conn.res_query_clients().ok()?.reply().ok()?.clients;
    let others = clients.iter().filter(|c| c.resource_base != 0 && c.resource_base != me).count();
    // One of them is our window manager.
    Some(others <= 1)
}
