//! Nested mode: ask the host desktop (GNOME, KDE, ...) to stop handling its
//! own shortcuts while the WhiteRose window is focused, through
//! keyboard-shortcuts-inhibit-unstable-v1, the protocol VM and remote-desktop
//! viewers use. Then SUPER, ALT+TAB, CTRL+ALT+TAB and PrtSc reach WhiteRose
//! instead of the host. GNOME asks the user once to allow it.

use std::ptr::NonNull;

use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use wayland_client::{
    backend::{Backend, ObjectId},
    globals::{registry_queue_init, GlobalListContents},
    protocol::{wl_registry, wl_seat::WlSeat, wl_surface::WlSurface},
    Connection, Dispatch, EventQueue, Proxy, QueueHandle,
};
use wayland_protocols::wp::keyboard_shortcuts_inhibit::zv1::client::{
    zwp_keyboard_shortcuts_inhibit_manager_v1::ZwpKeyboardShortcutsInhibitManagerV1,
    zwp_keyboard_shortcuts_inhibitor_v1::{self, ZwpKeyboardShortcutsInhibitorV1},
};

#[derive(Default)]
struct HostState {
    active: bool,
}

impl Dispatch<wl_registry::WlRegistry, GlobalListContents> for HostState {
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
impl Dispatch<ZwpKeyboardShortcutsInhibitManagerV1, ()> for HostState {
    fn event(
        _: &mut Self,
        _: &ZwpKeyboardShortcutsInhibitManagerV1,
        _: <ZwpKeyboardShortcutsInhibitManagerV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<WlSeat, ()> for HostState {
    fn event(_: &mut Self, _: &WlSeat, _: <WlSeat as Proxy>::Event, _: &(), _: &Connection, _: &QueueHandle<Self>) {}
}
impl Dispatch<ZwpKeyboardShortcutsInhibitorV1, ()> for HostState {
    fn event(
        state: &mut Self,
        _: &ZwpKeyboardShortcutsInhibitorV1,
        event: zwp_keyboard_shortcuts_inhibitor_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            zwp_keyboard_shortcuts_inhibitor_v1::Event::Active => {
                if !state.active {
                    tracing::info!("host shortcuts inhibited: SUPER, ALT+TAB and PrtSc now reach WhiteRose");
                }
                state.active = true;
            }
            zwp_keyboard_shortcuts_inhibitor_v1::Event::Inactive => state.active = false,
            _ => {}
        }
    }
}

/// Keeps the inhibitor alive; dropping it gives the shortcuts back to the host.
pub struct HostInhibit {
    queue: EventQueue<HostState>,
    state: HostState,
    _inhibitor: ZwpKeyboardShortcutsInhibitorV1,
}

impl HostInhibit {
    /// Requests the inhibit for the nested window. Returns None (with the
    /// reason logged) if the host isn't Wayland or doesn't support it.
    pub fn request(window: &(impl HasWindowHandle + HasDisplayHandle)) -> Option<HostInhibit> {
        let (display, surface) =
            match (window.display_handle().map(|h| h.as_raw()), window.window_handle().map(|h| h.as_raw())) {
                (Ok(RawDisplayHandle::Wayland(d)), Ok(RawWindowHandle::Wayland(w))) => (d.display, w.surface),
                _ => {
                    tracing::info!("host is not Wayland; its shortcuts can't be inhibited");
                    return None;
                }
            };
        match Self::connect(display, surface) {
            Ok(h) => Some(h),
            Err(e) => {
                tracing::warn!(e, "host shortcuts not inhibited: the host keeps SUPER, ALT+TAB and PrtSc");
                None
            }
        }
    }

    fn connect(display: NonNull<std::ffi::c_void>, surface: NonNull<std::ffi::c_void>) -> Result<HostInhibit, String> {
        // Safety: winit owns this wl_display and keeps it alive for the window's lifetime,
        // which outlives this struct (both live until the compositor exits).
        let backend = unsafe { Backend::from_foreign_display(display.as_ptr().cast()) };
        let conn = Connection::from_backend(backend);
        let (globals, mut queue) = registry_queue_init::<HostState>(&conn).map_err(|e| e.to_string())?;
        let qh = queue.handle();
        let manager: ZwpKeyboardShortcutsInhibitManagerV1 =
            globals.bind(&qh, 1..=1, ()).map_err(|_| "host has no keyboard-shortcuts-inhibit".to_string())?;
        let seat: WlSeat = globals.bind(&qh, 1..=1, ()).map_err(|_| "host has no wl_seat".to_string())?;
        // Safety: the pointer is winit's live wl_surface proxy for our window.
        let id = unsafe { ObjectId::from_ptr(WlSurface::interface(), surface.as_ptr().cast()) }
            .map_err(|e| e.to_string())?;
        let wl_surface = WlSurface::from_id(&conn, id).map_err(|e| e.to_string())?;
        let inhibitor = manager.inhibit_shortcuts(&wl_surface, &seat, &qh, ());
        let mut state = HostState::default();
        queue.roundtrip(&mut state).map_err(|e| e.to_string())?;
        tracing::info!("asked the host to pass its shortcuts through (GNOME asks once to allow this)");
        Ok(HostInhibit { queue, state, _inhibitor: inhibitor })
    }

    /// Handles active/inactive notices; call once per frame.
    pub fn dispatch(&mut self) {
        let _ = self.queue.dispatch_pending(&mut self.state);
    }
}
