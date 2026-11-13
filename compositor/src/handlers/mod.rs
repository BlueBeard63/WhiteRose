//! Wayland protocol handlers. Based on Smithay's smallvil example (MIT).

mod compositor;
mod dmabuf;
mod layer_shell;
mod xdg_shell;

use smithay::{
    delegate_data_device, delegate_output, delegate_seat,
    input::{Seat, SeatHandler, SeatState},
    reexports::wayland_server::{protocol::wl_surface::WlSurface, Resource},
    wayland::{
        output::OutputHandler,
        selection::{
            data_device::{
                set_data_device_focus, ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
            },
            SelectionHandler,
        },
    },
};

use crate::state::WhiteRose;

impl SeatHandler for WhiteRose {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<WhiteRose> {
        &mut self.seat_state
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, image: smithay::input::pointer::CursorImageStatus) {
        // Redraw to show it (nested: to pass the shape on to the host).
        if self.cursor_status != image {
            self.cursor_status = image;
            self.schedule_redraw();
        }
    }

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        tracing::debug!(surface = ?focused.map(|s| s.id()), "keyboard focus");
        let dh = &self.display_handle;
        let client = focused.and_then(|s| dh.get_client(s.id()).ok());
        set_data_device_focus(dh, seat, client);
    }
}
delegate_seat!(WhiteRose);

impl SelectionHandler for WhiteRose {
    /// Selections the compositor itself offers: its own data (a screenshot,
    /// a history entry pasted back), or an X11 app's copy.
    type SelectionUserData = crate::clipboard_capture::Offer;

    fn new_selection(
        &mut self,
        ty: smithay::wayland::selection::SelectionTarget,
        source: Option<smithay::wayland::selection::SelectionSource>,
        _seat: Seat<Self>,
    ) {
        // X11 apps paste it too (now, or when Xwayland starts).
        if ty == smithay::wayland::selection::SelectionTarget::Clipboard {
            self.xwayland.clipboard = source.as_ref().map(|s| s.mime_types());
        }
        if let Some(wm) = self.xwayland.wm.as_mut() {
            let _ = wm.new_selection(ty, source.as_ref().map(|s| s.mime_types()));
        }
        // Only the real clipboard (not the middle-click primary selection),
        // and only copies made by apps.
        if ty == smithay::wayland::selection::SelectionTarget::Clipboard {
            if let Some(source) = source {
                // Smithay calls this *before* it records the new selection, so
                // reading now would read the previous one. Read once it's set.
                let mimes = source.mime_types();
                self.loop_handle.insert_idle(move |state| state.capture_clipboard(mimes));
            }
        }
    }

    fn send_selection(
        &mut self,
        ty: smithay::wayland::selection::SelectionTarget,
        mime_type: String,
        fd: std::os::fd::OwnedFd,
        _seat: Seat<Self>,
        user_data: &Self::SelectionUserData,
    ) {
        use crate::clipboard_capture::Offer;
        match user_data {
            Offer::Data(data) => crate::clipboard_capture::write_offer(data.clone(), mime_type, fd),
            Offer::X11 => {
                let handle = self.loop_handle.clone();
                if let Some(wm) = self.xwayland.wm.as_mut() {
                    if let Err(err) = wm.send_selection(ty, mime_type, fd, handle) {
                        tracing::debug!(%err, "couldn't read the X11 clipboard");
                    }
                }
            }
        }
    }
}

impl DataDeviceHandler for WhiteRose {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}
impl ClientDndGrabHandler for WhiteRose {
    fn started(
        &mut self,
        _source: Option<smithay::reexports::wayland_server::protocol::wl_data_source::WlDataSource>,
        icon: Option<WlSurface>,
        _seat: Seat<Self>,
    ) {
        self.dnd_icon = icon;
        self.schedule_redraw();
    }

    fn dropped(&mut self, _target: Option<WlSurface>, _validated: bool, _seat: Seat<Self>) {
        self.dnd_icon = None;
        self.schedule_redraw();
    }

    // A drag over an X11 window goes to it through XDND (xdnd.rs).
    fn dnd_foreign(&mut self, surface: &WlSurface) -> bool {
        self.dnd_is_x11(surface)
    }

    fn dnd_foreign_motion(
        &mut self,
        surface: &WlSurface,
        location: smithay::utils::Point<f64, smithay::utils::Logical>,
        source: &smithay::reexports::wayland_server::protocol::wl_data_source::WlDataSource,
        _seat: Seat<Self>,
    ) {
        self.xdnd_motion(surface, location, source);
    }

    fn dnd_foreign_leave(&mut self, _seat: Seat<Self>) {
        self.xdnd_leave();
    }

    fn dnd_foreign_drop(
        &mut self,
        _source: &smithay::reexports::wayland_server::protocol::wl_data_source::WlDataSource,
        _seat: Seat<Self>,
    ) -> bool {
        self.xdnd_drop()
    }
}
/// The compositor's own drags: an X11 app's carried on into Wayland ones
/// (xdnd.rs).
impl ServerDndGrabHandler for WhiteRose {
    fn accept(&mut self, mime_type: Option<String>, _seat: Seat<Self>) {
        self.xdnd_in_accept(mime_type);
    }

    fn action(
        &mut self,
        action: smithay::reexports::wayland_server::protocol::wl_data_device_manager::DndAction,
        _seat: Seat<Self>,
    ) {
        self.xdnd_in_action(action);
    }

    fn send(&mut self, mime_type: String, fd: std::os::fd::OwnedFd, _seat: Seat<Self>) {
        self.xdnd_in_send(mime_type, fd);
    }

    fn cancelled(&mut self, _seat: Seat<Self>) {
        self.xdnd_in_result(false);
    }

    fn finished(&mut self, _seat: Seat<Self>) {
        self.xdnd_in_result(true);
    }
}
delegate_data_device!(WhiteRose);

impl OutputHandler for WhiteRose {}
delegate_output!(WhiteRose);

impl smithay::wayland::fractional_scale::FractionalScaleHandler for WhiteRose {
    /// Tell a new client its scale before it draws its first frame (after
    /// that, rendering keeps it up to date; see render::send_scale).
    fn new_fractional_scale(&mut self, surface: WlSurface) {
        let output = self.space.outputs().next().cloned();
        if let Some(output) = output {
            crate::render::send_scale(&surface, &output);
        }
    }
}
smithay::delegate_fractional_scale!(WhiteRose);
smithay::delegate_viewporter!(WhiteRose);

// wp_cursor_shape_v1: apps name the cursor they want ("text", "pointer")
// and we draw it from our theme, the same size everywhere, instead of each
// app loading and scaling its own.
impl smithay::wayland::tablet_manager::TabletSeatHandler for WhiteRose {}
smithay::delegate_cursor_shape!(WhiteRose);
