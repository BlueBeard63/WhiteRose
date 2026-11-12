//! One way to talk to a window, whatever kind it is: its ids, the size and
//! states it's told, closing it, its size limits, its surface and client.
//! Everything outside the shell handlers goes through this rather than a
//! window's Wayland toplevel, so an X11 window (XWayland) can answer too
//! (docs/plans/xwayland.md).
//!
//! An X11 window has no "pending, then configure": its next size and states
//! are kept with it (`X11Next`), `configure` marks them to send, and the
//! compositor sends them with its place after each batch of events
//! (`xwayland.rs`, `sync_x11`), as X11 wants both together.

use std::cell::RefCell;

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
    xwayland::xwm::WmWindowType,
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
#[derive(Debug, Clone, Default)]
pub struct Pending {
    pub size: Option<Size<i32, Logical>>,
    states: Vec<ToplevelState>,
}

impl PartialEq for Pending {
    fn eq(&self, o: &Self) -> bool {
        self.size == o.size && self.states.len() == o.states.len() && self.states.iter().all(|s| o.states.contains(s))
    }
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

/// Whether `w` is an X11 window (XWayland).
pub fn is_x11(w: &Window) -> bool {
    w.x11_surface().is_some()
}

/// An X11 window's next size and states, and whether they're to be sent.
#[derive(Debug, Default)]
pub struct X11Next(pub RefCell<(Pending, bool)>);

fn x11_next<R>(w: &Window, f: impl FnOnce(&mut (Pending, bool)) -> R) -> R {
    let next = w.user_data().get_or_insert(X11Next::default);
    let mut n = next.0.borrow_mut();
    f(&mut n)
}

/// An X11 window's next size and states if they're to be sent (and from
/// now on they're not), for `sync_x11`; else None.
pub fn take_x11_next(w: &Window) -> Option<Pending> {
    x11_next(w, |n| std::mem::take(&mut n.1).then(|| n.0.clone()))
}

/// The size an X11 window's been told (None: its own).
pub fn x11_size(w: &Window) -> Option<Size<i32, Logical>> {
    x11_next(w, |n| n.0.size)
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
        if let Some(x) = self.x11_surface() {
            return x.wl_surface();
        }
        self.toplevel().map(|t| t.wl_surface().clone())
    }

    fn ids(&self) -> (Option<String>, Option<String>) {
        if let Some(x) = self.x11_surface() {
            // WM_CLASS's class ("Spotify"), else its instance, as the app id.
            let app =
                Some(x.class()).filter(|c| !c.is_empty()).or_else(|| Some(x.instance()).filter(|i| !i.is_empty()));
            return (app, Some(x.title()).filter(|t| !t.is_empty()));
        }
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
        if let Some(x) = self.x11_surface() {
            return x.is_transient_for().is_some()
                || matches!(
                    x.window_type(),
                    Some(WmWindowType::Dialog | WmWindowType::Utility | WmWindowType::Toolbar | WmWindowType::Splash)
                );
        }
        self.toplevel().is_some_and(|t| t.parent().is_some())
    }

    fn size_limits(&self) -> (Size<i32, Logical>, Size<i32, Logical>) {
        if let Some(x) = self.x11_surface() {
            return (x.min_size().unwrap_or_default(), x.max_size().unwrap_or_default());
        }
        let Some(t) = self.toplevel() else { return Default::default() };
        with_states(t.wl_surface(), |s| {
            let mut cached = s.cached_state.get::<SurfaceCachedState>();
            let c = cached.current();
            (c.min_size, c.max_size)
        })
    }

    fn pending_size(&self) -> Option<Size<i32, Logical>> {
        if is_x11(self) {
            return x11_size(self);
        }
        self.toplevel().and_then(|t| t.with_pending_state(|s| s.size))
    }

    fn has(&self, state: ToplevelState) -> bool {
        if let Some(x) = self.x11_surface() {
            // Activation goes straight to X (`Window::set_activated`).
            if state == ToplevelState::Activated {
                return x.is_activated();
            }
            return x11_next(self, |n| n.0.contains(state));
        }
        self.toplevel().is_some_and(|t| t.with_pending_state(|s| s.states.contains(state)))
    }

    fn pend(&self, f: impl FnOnce(&mut Pending)) -> bool {
        if is_x11(self) {
            return x11_next(self, |n| {
                let before = n.0.clone();
                f(&mut n.0);
                n.0 != before
            });
        }
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
        if is_x11(self) {
            x11_next(self, |n| n.1 = true);
            return;
        }
        if let Some(t) = self.toplevel() {
            t.send_pending_configure();
        }
    }

    fn close(&self) {
        if let Some(x) = self.x11_surface() {
            let _ = x.close();
            return;
        }
        if let Some(t) = self.toplevel() {
            t.send_close();
        }
    }

    fn pid(&self, dh: &DisplayHandle) -> Option<u32> {
        if let Some(x) = self.x11_surface() {
            return x.pid();
        }
        let client = self.toplevel()?.wl_surface().client()?;
        let creds = client.get_credentials(dh).ok()?;
        u32::try_from(creds.pid).ok()
    }
}
