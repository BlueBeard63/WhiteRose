//! Helpers for the tiling tests: the default dwindle tiles below the bar,
//! reading where windows are, and opening described windows.

use serde_json::Value;
use smithay::{desktop::Window, reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as ServerState};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_protocols::xdg::shell::client::xdg_toplevel::XdgToplevel;
use whiterose_wm::WhiteRose;

use serde_json::json;
use smithay::{
    backend::input::{ButtonState, KeyState},
    input::keyboard::Keycode,
};

use super::{Client, Headless};

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

pub const BTN_LEFT: u32 = 0x110;
pub const BTN_RIGHT: u32 = 0x111;
pub const SUPER: u32 = 125 + 8;

pub fn super_key(s: &mut WhiteRose, down: bool) {
    s.on_key(Keycode::new(SUPER), if down { KeyState::Pressed } else { KeyState::Released }, 0);
}

/// Presses `button` at `from`, moves to `to`, and lets go; returns the drop
/// preview seen just before letting go.
pub fn drag(s: &mut WhiteRose, button: u32, from: (f64, f64), to: (f64, f64)) -> Option<(i32, i32, i32, i32)> {
    s.on_pointer_motion(from.into(), 1);
    s.on_pointer_button(button, ButtonState::Pressed, 2);
    s.on_pointer_motion(((from.0 + to.0) / 2.0, (from.1 + to.1) / 2.0).into(), 3);
    s.on_pointer_motion(to.into(), 4);
    let preview = s.tiling.preview.rect.map(|r| (r.loc.x, r.loc.y, r.size.w, r.size.h));
    s.on_pointer_button(button, ButtonState::Released, 5);
    preview
}

pub fn super_drag(s: &mut WhiteRose, button: u32, from: (f64, f64), to: (f64, f64)) -> Option<(i32, i32, i32, i32)> {
    super_key(s, true);
    let preview = drag(s, button, from, to);
    super_key(s, false);
    preview
}

/// A bar and two dwindle tiles that have drawn at their size, so the
/// pointer finds them.
pub fn two_tiles(h: &mut Headless, test: impl FnOnce(&mut WhiteRose) + Send + 'static) {
    set(&mut h.state, "windows.layout", json!("dwindle"));
    h.drive(move |conn, ask| {
        let mut c = Client::connect(&conn);
        c.open_bar();
        let (a, _ta) = open(&mut c);
        let (b, _tb) = open(&mut c);
        c.attach_buffer(&a, LEFT.2, LEFT.3);
        c.attach_buffer(&b, RIGHT.2, RIGHT.3);
        ask.run(move |s| {
            assert_eq!(rects(s), vec![Some(LEFT), Some(RIGHT)]);
            test(s);
        });
    });
}
