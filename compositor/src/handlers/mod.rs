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

    fn cursor_image(&mut self, _seat: &Seat<Self>, _image: smithay::input::pointer::CursorImageStatus) {}

    fn focus_changed(&mut self, seat: &Seat<Self>, focused: Option<&WlSurface>) {
        let dh = &self.display_handle;
        let client = focused.and_then(|s| dh.get_client(s.id()).ok());
        set_data_device_focus(dh, seat, client);
    }
}
delegate_seat!(WhiteRose);

impl SelectionHandler for WhiteRose {
    /// Data for selections the compositor itself offers (a screenshot PNG).
    type SelectionUserData = std::sync::Arc<Vec<u8>>;

    fn send_selection(
        &mut self,
        _ty: smithay::wayland::selection::SelectionTarget,
        _mime_type: String,
        fd: std::os::fd::OwnedFd,
        _seat: Seat<Self>,
        user_data: &Self::SelectionUserData,
    ) {
        // Write on a thread: the reader may be slow and the pipe small.
        let data = user_data.clone();
        std::thread::spawn(move || {
            use std::io::Write;
            let mut f = std::fs::File::from(fd);
            if let Err(e) = f.write_all(&data) {
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
impl ClientDndGrabHandler for WhiteRose {}
impl ServerDndGrabHandler for WhiteRose {}
delegate_data_device!(WhiteRose);

impl OutputHandler for WhiteRose {}
delegate_output!(WhiteRose);
