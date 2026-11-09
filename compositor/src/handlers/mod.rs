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
    /// Selections the compositor itself offers (a screenshot, a history
    /// entry pasted back): the data for each offered mime type.
    type SelectionUserData = std::sync::Arc<Vec<(String, Vec<u8>)>>;

    fn new_selection(
        &mut self,
        ty: smithay::wayland::selection::SelectionTarget,
        source: Option<smithay::wayland::selection::SelectionSource>,
        _seat: Seat<Self>,
    ) {
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
        _ty: smithay::wayland::selection::SelectionTarget,
        mime_type: String,
        fd: std::os::fd::OwnedFd,
        _seat: Seat<Self>,
        user_data: &Self::SelectionUserData,
    ) {
        // Write on a thread: the reader may be slow and the pipe small.
        let data = user_data.clone();
        std::thread::spawn(move || {
            use std::io::Write;
            let bytes =
                data.iter().find(|(m, _)| *m == mime_type).or(data.first()).map(|(_, b)| b.as_slice()).unwrap_or(&[]);
            let mut f = std::fs::File::from(fd);
            if let Err(e) = f.write_all(bytes) {
                tracing::debug!(%e, "clipboard reader went away");
            }
        });
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
}
impl ServerDndGrabHandler for WhiteRose {}
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
