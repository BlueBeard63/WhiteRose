//! One way to talk to a window, whatever kind it is: its ids, the size and
//! states it's told, closing it, its size limits, its surface and client.
//! Everything outside the shell handlers goes through this rather than a
//! window's Wayland toplevel, so an X11 window (XWayland) can answer too
//! (docs/plans/xwayland.md).

use smithay::{
    desktop::Window,
    reexports::{
        wayland_protocols::xdg::shell::server::xdg_toplevel::State as ToplevelState,
        wayland_server::{protocol::wl_surface::WlSurface, DisplayHandle, Resource},
    },
    utils::{Logical, Size},
    wayland::{
        compositor::with_states,
        shell::xdg::{SurfaceCachedState, XdgToplevelSurfaceData},
    },
};

/// The states a window is told that the compositor changes.
const KNOWN: [ToplevelState; 8] = [
    ToplevelState::TiledLeft,
    ToplevelState::TiledRight,
    ToplevelState::TiledTop,
    ToplevelState::TiledBottom,
    ToplevelState::Maximized,
    ToplevelState::Fullscreen,
    ToplevelState::Activated,
    ToplevelState::Resizing,
];

/// A window's next size and states, to change in one go (`WinExt::pend`).
#[derive(Debug, Clone, PartialEq)]
pub struct Pending {
    pub size: Option<Size<i32, Logical>>,
    states: Vec<ToplevelState>,
}

impl Pending {
    pub fn set(&mut self, s: ToplevelState) {
        if !self.states.contains(&s) {
            self.states.push(s);
        }
    }
    pub fn unset(&mut self, s: ToplevelState) {
        self.states.retain(|x| *x != s);
    }
    pub fn contains(&self, s: ToplevelState) -> bool {
        self.states.contains(&s)
    }
}

/// Whether `w` is an X11 window (XWayland). None yet: step 2.
pub fn is_x11(_w: &Window) -> bool {
    false
}

pub trait WinExt {
    /// The surface that takes focus and input.
    fn surface(&self) -> Option<WlSurface>;
    /// (app id, title).
    fn ids(&self) -> (Option<String>, Option<String>);
    /// A dialog or another window's child (it floats).
    fn is_child(&self) -> bool;
    /// Its smallest and largest size (0: no limit).
    fn size_limits(&self) -> (Size<i32, Logical>, Size<i32, Logical>);
    /// The size it's been told (or will be at the next configure).
    fn pending_size(&self) -> Option<Size<i32, Logical>>;
    /// Whether it's been told `state`.
    fn has(&self, state: ToplevelState) -> bool;
    /// Changes its next size and states; true if anything changed. Sent with
    /// `configure`.
    fn pend(&self, f: impl FnOnce(&mut Pending)) -> bool;
    /// Sends what's pending.
    fn configure(&self);
    /// Asks it to close.
    fn close(&self);
    /// Its client's process id.
    fn pid(&self, dh: &DisplayHandle) -> Option<u32>;
}

impl WinExt for Window {
    fn surface(&self) -> Option<WlSurface> {
        self.toplevel().map(|t| t.wl_surface().clone())
    }

    fn ids(&self) -> (Option<String>, Option<String>) {
        let Some(t) = self.toplevel() else { return (None, None) };
        with_states(t.wl_surface(), |states| {
            states
                .data_map
                .get::<XdgToplevelSurfaceData>()
                .map(|d| {
                    let d = d.lock().unwrap();
                    (d.app_id.clone(), d.title.clone())
                })
                .unwrap_or_default()
        })
    }

    fn is_child(&self) -> bool {
        self.toplevel().is_some_and(|t| t.parent().is_some())
    }

    fn size_limits(&self) -> (Size<i32, Logical>, Size<i32, Logical>) {
        let Some(t) = self.toplevel() else { return Default::default() };
        with_states(t.wl_surface(), |s| {
            let mut cached = s.cached_state.get::<SurfaceCachedState>();
            let c = cached.current();
            (c.min_size, c.max_size)
        })
    }

    fn pending_size(&self) -> Option<Size<i32, Logical>> {
        self.toplevel().and_then(|t| t.with_pending_state(|s| s.size))
    }

    fn has(&self, state: ToplevelState) -> bool {
        self.toplevel().is_some_and(|t| t.with_pending_state(|s| s.states.contains(state)))
    }

    fn pend(&self, f: impl FnOnce(&mut Pending)) -> bool {
        let Some(t) = self.toplevel() else { return false };
        t.with_pending_state(|s| {
            let before =
                Pending { size: s.size, states: KNOWN.into_iter().filter(|k| s.states.contains(*k)).collect() };
            let mut p = before.clone();
            f(&mut p);
            s.size = p.size;
            for k in KNOWN {
                if p.contains(k) {
                    s.states.set(k);
                } else {
                    s.states.unset(k);
                }
            }
            p.size != before.size || KNOWN.iter().any(|k| p.contains(*k) != before.contains(*k))
        })
    }

    fn configure(&self) {
        if let Some(t) = self.toplevel() {
            t.send_pending_configure();
        }
    }

    fn close(&self) {
        if let Some(t) = self.toplevel() {
            t.send_close();
        }
    }

    fn pid(&self, dh: &DisplayHandle) -> Option<u32> {
        let client = self.toplevel()?.wl_surface().client()?;
        let creds = client.get_credentials(dh).ok()?;
        u32::try_from(creds.pid).ok()
    }
}
