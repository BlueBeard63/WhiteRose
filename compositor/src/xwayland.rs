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
    desktop::Window,
    input::pointer::GrabStartData as PointerGrabStartData,
    reexports::{calloop::LoopHandle, wayland_protocols::xdg::shell::server::xdg_toplevel::State as ToplevelState},
    utils::{Logical, Rectangle, SERIAL_COUNTER},
    wayland::{
        selection::SelectionTarget,
        xwayland_shell::{XWaylandShellHandler, XWaylandShellState},
    },
    xwayland::{
        xwm::{Reorder, ResizeEdge as X11ResizeEdge, XwmId},
        X11Surface, X11Wm, XWayland, XWaylandEvent, XwmHandler,
    },
};

use crate::{state::WhiteRose, win::WinExt};

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
    /// The display it serves (":N").
    pub display: Option<u32>,
}

impl XState {
    pub fn new(dh: &smithay::reexports::wayland_server::DisplayHandle) -> Self {
        XState { shell: XWaylandShellState::new::<WhiteRose>(dh), wm: None, display: None }
    }
}

impl WhiteRose {
    /// Starts Xwayland and sets DISPLAY for what the compositor starts from
    /// now on. Without an Xwayland, X11 apps just don't open.
    pub fn start_xwayland(&mut self, handle: &LoopHandle<'static, WhiteRose>) {
        let spawned = XWayland::spawn(
            &self.display_handle,
            None,
            std::iter::empty::<(String, String)>(),
            true,
            std::process::Stdio::null(),
            std::process::Stdio::null(),
            |_| {},
        );
        let (xwayland, client) = match spawned {
            Ok(x) => x,
            Err(err) => {
                tracing::warn!(%err, "no XWayland: X11 apps won't open");
                return;
            }
        };
        let display = xwayland.display_number();
        self.xwayland.display = Some(display);
        // (Before the daemon and shell start: they and what they launch inherit it.)
        std::env::set_var("DISPLAY", format!(":{display}"));
        let wm_handle = handle.clone();
        let inserted = handle.insert_source(xwayland, move |event, _, state| match event {
            XWaylandEvent::Ready { x11_socket, display_number } => {
                match X11Wm::start_wm(wm_handle.clone(), x11_socket, client.clone()) {
                    Ok(wm) => {
                        tracing::info!(display = display_number, "XWayland is running");
                        state.xwayland.wm = Some(wm);
                    }
                    Err(err) => tracing::warn!(%err, "the X11 window manager didn't start"),
                }
            }
            XWaylandEvent::Error => tracing::warn!("XWayland stopped"),
        });
        if let Err(err) = inserted {
            tracing::warn!(%err, "XWayland's events couldn't be watched");
        }
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
}
smithay::delegate_xwayland_shell!(WhiteRose);

impl XwmHandler for WhiteRose {
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

    // Copy and paste between X11 and Wayland: step 4.
    fn allow_selection_access(&mut self, _: XwmId, _: SelectionTarget) -> bool {
        false
    }
}
