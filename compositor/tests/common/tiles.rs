//! Helpers for the tiling tests: the default dwindle tiles below the bar,
//! reading where windows are, and opening described windows.

use serde_json::Value;
use smithay::{desktop::Window, reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as ServerState};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_protocols::xdg::shell::client::xdg_toplevel::XdgToplevel;
use whiterose_wm::WhiteRose;

use super::Client;

/// The area below the bar, less the 8px outer gap.
pub const AREA: (i32, i32, i32, i32) = (8, 34, 1424, 766);
/// Its two halves, with the 8px gap between them.
pub const LEFT: (i32, i32, i32, i32) = (8, 34, 708, 766);
pub const RIGHT: (i32, i32, i32, i32) = (724, 34, 708, 766);

pub fn set(state: &mut WhiteRose, key: &str, value: Value) {
    if state.layout_setting(key, &value) {
        state.apply_layout_settings();
    }
}

/// Every window, oldest first.
pub fn all(state: &WhiteRose) -> Vec<Window> {
    let mut ws: Vec<Window> = state.workspaces.mru().to_vec();
    ws.sort_by_key(|w| state.window_id(w));
    ws
}

/// Where a window is and the size it was told to take (0x0: its own
/// choice); None while it's hidden.
pub fn rect(state: &WhiteRose, w: &Window) -> Option<(i32, i32, i32, i32)> {
    let loc = state.space.element_location(w)?;
    let size = w.toplevel().unwrap().with_pending_state(|s| s.size).unwrap_or_default();
    Some((loc.x, loc.y, size.w, size.h))
}

pub fn rects(state: &WhiteRose) -> Vec<Option<(i32, i32, i32, i32)>> {
    all(state).iter().map(|w| rect(state, w)).collect()
}

pub fn told_tiled(w: &Window) -> bool {
    w.toplevel().unwrap().with_pending_state(|s| s.states.contains(ServerState::TiledLeft))
}

/// Opens a window, letting `setup` describe it before its first commit.
pub fn open_with(c: &mut Client, setup: impl FnOnce(&XdgToplevel)) -> (WlSurface, XdgToplevel) {
    let qh = c.queue.handle();
    let surface = c.compositor.create_surface(&qh, ());
    let xdg_surface = c.xdg.get_xdg_surface(&surface, &qh, ());
    let toplevel = xdg_surface.get_toplevel(&qh, ());
    setup(&toplevel);
    surface.commit();
    c.app.toplevel_configured = false;
    c.until(|a| a.toplevel_configured);
    (surface, toplevel)
}

pub fn open(c: &mut Client) -> (WlSurface, XdgToplevel) {
    open_with(c, |_| {})
}
