// Adapted from Smithay's smallvil example (MIT, (c) Victor Berger and Victoria Brekenfeld).
use smithay::{
    delegate_xdg_shell,
    desktop::{find_popup_root_surface, get_popup_toplevel_coords, PopupKind, PopupManager, Space, Window},
    input::{
        pointer::{Focus, GrabStartData as PointerGrabStartData},
        Seat,
    },
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel,
        wayland_server::{
            protocol::{wl_output, wl_seat, wl_surface::WlSurface},
            Resource,
        },
    },
    utils::{Rectangle, Serial},
    wayland::{
        compositor::with_states,
        shell::xdg::{
            PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState, XdgToplevelSurfaceData,
        },
    },
};

use crate::{
    grabs::{resize_grab::ResizeEdge, MoveSurfaceGrab, ResizeSurfaceGrab},
    WhiteRose,
};

impl XdgShellHandler for WhiteRose {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let window = Window::new_wayland_window(surface);
        // It opens on the output under the pointer. Its first commit applies
        // the window rules, which may move it, then leaves fullscreen on the
        // output it ends up on (window_rules.rs).
        let output = self.pointer_output();
        // Cascade new windows so they don't stack exactly on top of each other.
        // On a tiled workspace this is where it floats if it's untiled later;
        // its first commit gives it a tile (tiling.rs).
        let n = self.workspaces.active_windows().len() as i32;
        // Start inside the area the bar doesn't reserve.
        let area = output.map(|o| self.usable_area_on(&o)).unwrap_or_default().loc;
        let loc = (area.x + 48 + 32 * (n % 8), area.y + 48 + 32 * (n % 8));
        self.space.map_element(window.clone(), loc, false);
        self.assign_window_id(&window);
        self.workspaces.add(window.clone());
        tracing::info!(workspace = self.workspaces.active() + 1, at = ?loc, "window opened");
        self.focus_window(Some(window));
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        if let Some(window) = self.window_for_surface(surface.wl_surface()) {
            self.forget_window(&window);
            tracing::info!("window closed");
        }
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        self.unconstrain_popup(&surface);
        let _ = self.popups.track_popup(PopupKind::Xdg(surface));
    }

    fn reposition_request(&mut self, surface: PopupSurface, positioner: PositionerState, token: u32) {
        surface.with_pending_state(|state| {
            let geometry = positioner.get_geometry();
            state.geometry = geometry;
            state.positioner = positioner;
        });
        self.unconstrain_popup(&surface);
        surface.send_repositioned(token);
    }

    fn move_request(&mut self, surface: ToplevelSurface, seat: wl_seat::WlSeat, serial: Serial) {
        let seat = Seat::from_resource(&seat).unwrap();

        let wl_surface = surface.wl_surface();

        let Some(start_data) = check_grab(&seat, wl_surface, serial) else { return };
        let Some(window) = self.window_for_surface(wl_surface) else { return };
        self.start_move(window, start_data, serial);
    }

    fn resize_request(
        &mut self,
        surface: ToplevelSurface,
        seat: wl_seat::WlSeat,
        serial: Serial,
        edges: xdg_toplevel::ResizeEdge,
    ) {
        let seat = Seat::from_resource(&seat).unwrap();

        let wl_surface = surface.wl_surface();

        let Some(start_data) = check_grab(&seat, wl_surface, serial) else { return };
        let Some(window) = self.window_for_surface(wl_surface) else { return };
        self.start_resize(window, edges.into(), start_data, serial);
    }

    fn app_id_changed(&mut self, _surface: ToplevelSurface) {
        self.ipc_sync();
    }

    fn title_changed(&mut self, _surface: ToplevelSurface) {
        self.ipc_sync();
        // Tab strips show titles.
        if !self.tiling.strips.is_empty() {
            self.schedule_redraw();
        }
    }

    fn maximize_request(&mut self, surface: ToplevelSurface) {
        if let Some(w) = self.window_for_surface(surface.wl_surface()) {
            self.maximize(&w);
        } else {
            // Before the window is mapped: just acknowledge so the client isn't left waiting.
            surface.send_configure();
        }
    }

    fn minimize_request(&mut self, surface: ToplevelSurface) {
        if let Some(w) = self.window_for_surface(surface.wl_surface()) {
            self.minimize(&w);
        }
    }

    fn unmaximize_request(&mut self, surface: ToplevelSurface) {
        if let Some(w) = self.window_for_surface(surface.wl_surface()) {
            self.unmaximize(&w);
        }
    }

    fn fullscreen_request(&mut self, surface: ToplevelSurface, output: Option<wl_output::WlOutput>) {
        // On the output asked for, else the one the window is on. A
        // scratchpad stays as it is (it's told so).
        if let Some(w) = self.window_for_surface(surface.wl_surface()).filter(|w| !self.scratchpads.is_pad(w)) {
            self.fullscreen_to(&w, output.as_ref().and_then(smithay::output::Output::from_resource));
        } else {
            surface.send_configure();
        }
    }

    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        if let Some(w) = self.window_for_surface(surface.wl_surface()) {
            self.unfullscreen(&w);
        }
    }

    fn grab(&mut self, _surface: PopupSurface, _seat: wl_seat::WlSeat, _serial: Serial) {
        // TODO popup grabs
    }
}

// Xdg Shell
delegate_xdg_shell!(WhiteRose);

fn check_grab(seat: &Seat<WhiteRose>, surface: &WlSurface, serial: Serial) -> Option<PointerGrabStartData<WhiteRose>> {
    let pointer = seat.get_pointer()?;

    // Check that this surface has a click grab.
    if !pointer.has_grab(serial) {
        return None;
    }

    let start_data = pointer.grab_start_data()?;

    let (focus, _) = start_data.focus.as_ref()?;
    // If the focus was for a different surface, ignore the request.
    if !focus.id().same_client_as(&surface.id()) {
        return None;
    }

    Some(start_data)
}

/// Should be called on `WlSurface::commit`
pub fn handle_commit(popups: &mut PopupManager, space: &Space<Window>, surface: &WlSurface) {
    // Handle toplevel commits.
    if let Some(window) = space.elements().find(|w| w.toplevel().is_some_and(|t| t.wl_surface() == surface)).cloned() {
        let initial_configure_sent = with_states(surface, |states| {
            states.data_map.get::<XdgToplevelSurfaceData>().unwrap().lock().unwrap().initial_configure_sent
        });

        if !initial_configure_sent {
            window.toplevel().unwrap().send_configure();
        }
    }

    // Handle popup commits.
    popups.commit(surface);
    if let Some(popup) = popups.find_popup(surface) {
        match popup {
            PopupKind::Xdg(ref xdg) => {
                if !xdg.is_initial_configure_sent() {
                    // NOTE: This should never fail as the initial configure is always
                    // allowed.
                    xdg.send_configure().expect("initial configure failed");
                }
            }
            PopupKind::InputMethod(ref _input_method) => {}
        }
    }
}

impl WhiteRose {
    /// Moves a window with the pointer (its title bar, or SUPER+drag): a
    /// tile is dragged to a new place; anything else moves freely.
    pub fn start_move(&mut self, window: Window, start_data: PointerGrabStartData<WhiteRose>, serial: Serial) {
        if self.is_tiled(&window) && !Self::maximized_or_fullscreen(&window) {
            self.start_tile_move(&window, start_data, serial);
            return;
        }
        let Some(pointer) = self.seat.get_pointer() else { return };
        let Some(initial_window_location) = self.space.element_location(&window) else { return };
        let grab = MoveSurfaceGrab { start_data, window, initial_window_location, unsnapped: false };
        pointer.set_grab(self, grab, serial, Focus::Clear);
    }

    /// Resizes a window from `edges` with the pointer (its border, or
    /// SUPER+right-drag): a tile moves the splits on those edges; anything
    /// else changes size.
    pub fn start_resize(
        &mut self,
        window: Window,
        edges: ResizeEdge,
        start_data: PointerGrabStartData<WhiteRose>,
        serial: Serial,
    ) {
        if self.is_tiled(&window) && !Self::maximized_or_fullscreen(&window) {
            self.start_tile_resize(&window, crate::grabs::tile_grab::edge_dirs(edges), start_data, serial);
            return;
        }
        // A snapped window moves the edges it shares with other snapped
        // windows (or its own, if it shares none).
        if self.snap_of(&window).is_some() && !Self::maximized_or_fullscreen(&window) {
            self.start_snap_resize(&window, crate::grabs::tile_grab::edge_dirs(edges), start_data, serial);
            return;
        }
        let Some(pointer) = self.seat.get_pointer() else { return };
        let Some(loc) = self.space.element_location(&window) else { return };
        let Some(surface) = window.toplevel().cloned() else { return };
        surface.with_pending_state(|state| {
            state.states.set(xdg_toplevel::State::Resizing);
        });
        surface.send_pending_configure();
        let rect = Rectangle::new(loc, window.geometry().size);
        let grab = ResizeSurfaceGrab::start(start_data, window, edges, rect);
        pointer.set_grab(self, grab, serial, Focus::Clear);
    }

    fn unconstrain_popup(&self, popup: &PopupSurface) {
        let Ok(root) = find_popup_root_surface(&PopupKind::Xdg(popup.clone())) else {
            return;
        };
        let Some(window) = self.space.elements().find(|w| w.toplevel().is_some_and(|t| t.wl_surface() == &root)) else {
            return;
        };

        // Kept on the output its window is on.
        let Some(output_geo) = self.output_of(window).and_then(|o| self.space.output_geometry(&o)) else {
            return;
        };
        let window_geo = self.space.element_geometry(window).unwrap();

        // The target geometry for the positioner should be relative to its parent's geometry, so
        // we will compute that here.
        let mut target = output_geo;
        target.loc -= get_popup_toplevel_coords(&PopupKind::Xdg(popup.clone()));
        target.loc -= window_geo.loc;

        popup.with_pending_state(|state| {
            state.geometry = state.positioner.get_unconstrained_geometry(target);
        });
    }
}
