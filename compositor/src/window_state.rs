//! Maximize and fullscreen.
//!
//! Maximized windows fill the area panels don't reserve (below the bar).
//! A fullscreen window covers the whole output, bar included; while one is
//! showing, the renderer draws only it plus overlay-layer surfaces (e.g. the
//! screenshot overlay), which is also the cheapest frame to compose.
//! Both remember the floating geometry and restore it.

use smithay::{
    desktop::Window,
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as ToplevelState,
    utils::{Logical, Rectangle},
};

use crate::state::WhiteRose;

impl WhiteRose {
    /// The fullscreen window on the visible workspace, if any.
    pub fn active_fullscreen(&self) -> Option<&Window> {
        self.fullscreen.as_ref().filter(|w| self.space.element_location(w).is_some())
    }

    fn output_rect(&self) -> Rectangle<i32, Logical> {
        self.space.outputs().next().and_then(|o| self.space.output_geometry(o)).unwrap_or_default()
    }

    fn remember_floating(&mut self, w: &Window) {
        if self.restore_geometry.iter().any(|(r, _)| r == w) {
            return; // already maximized/fullscreen: keep the original floating geometry
        }
        let loc = self.space.element_location(w).unwrap_or_default();
        self.restore_geometry.push((w.clone(), Rectangle::new(loc, w.geometry().size)));
    }

    fn has_state(w: &Window, state: ToplevelState) -> bool {
        w.toplevel().is_some_and(|t| t.with_pending_state(|s| s.states.contains(state)))
    }

    /// Applies `state` with `rect` as the window's size and position.
    fn apply(&mut self, w: &Window, state: ToplevelState, on: bool, rect: Option<Rectangle<i32, Logical>>) {
        let Some(toplevel) = w.toplevel() else { return };
        toplevel.with_pending_state(|s| {
            if on {
                s.states.set(state);
            } else {
                s.states.unset(state);
            }
            // A zero size lets the client choose (e.g. it never had a buffer).
            s.size = rect.map(|r| r.size).filter(|sz| sz.w > 0 && sz.h > 0);
        });
        toplevel.send_pending_configure();
        if let Some(r) = rect {
            self.space.map_element(w.clone(), r.loc, true);
        }
        self.island_sync();
        self.schedule_redraw();
    }

    fn take_floating(&mut self, w: &Window) -> Option<Rectangle<i32, Logical>> {
        let i = self.restore_geometry.iter().position(|(r, _)| r == w)?;
        Some(self.restore_geometry.swap_remove(i).1)
    }

    pub fn maximize(&mut self, w: &Window) {
        if Self::has_state(w, ToplevelState::Maximized) {
            return;
        }
        self.remember_floating(w);
        let area = self.usable_area();
        self.apply(w, ToplevelState::Maximized, true, Some(area));
    }

    pub fn unmaximize(&mut self, w: &Window) {
        if !Self::has_state(w, ToplevelState::Maximized) {
            return;
        }
        let rect = if Self::has_state(w, ToplevelState::Fullscreen) {
            None // still fullscreen; only clear the flag
        } else {
            self.take_floating(w)
        };
        self.apply(w, ToplevelState::Maximized, false, rect);
    }

    pub fn fullscreen(&mut self, w: &Window) {
        if Self::has_state(w, ToplevelState::Fullscreen) {
            return;
        }
        // Only one fullscreen window at a time.
        if let Some(prev) = self.fullscreen.clone().filter(|p| p != w) {
            self.unfullscreen(&prev);
        }
        self.remember_floating(w);
        let rect = self.output_rect();
        self.apply(w, ToplevelState::Fullscreen, true, Some(rect));
        self.fullscreen = Some(w.clone());
        self.space.raise_element(w, true);
    }

    pub fn unfullscreen(&mut self, w: &Window) {
        if !Self::has_state(w, ToplevelState::Fullscreen) {
            return;
        }
        if self.fullscreen.as_ref() == Some(w) {
            self.fullscreen = None;
        }
        let rect = if Self::has_state(w, ToplevelState::Maximized) {
            Some(self.usable_area()) // back to maximized, not floating
        } else {
            self.take_floating(w)
        };
        self.apply(w, ToplevelState::Fullscreen, false, rect);
    }

    pub fn toggle_fullscreen(&mut self, w: &Window) {
        if Self::has_state(w, ToplevelState::Fullscreen) {
            self.unfullscreen(w);
        } else {
            self.fullscreen(w);
        }
    }

    /// Re-fits maximized and fullscreen windows after the output or the
    /// panels' reserved space changed (bar appeared, output resized).
    /// Cheap when nothing changed: the bar commits often (clock, EQ), so this
    /// only reconfigures windows when the geometry really moved.
    pub fn refit_windows(&mut self) {
        let area = self.usable_area();
        let output = self.output_rect();
        if self.fitted_to == Some((output, area)) {
            return;
        }
        self.fitted_to = Some((output, area));
        let windows: Vec<Window> = self.space.elements().cloned().collect();
        for w in windows {
            if Self::has_state(&w, ToplevelState::Fullscreen) {
                self.apply(&w, ToplevelState::Fullscreen, true, Some(output));
            } else if Self::has_state(&w, ToplevelState::Maximized) {
                self.apply(&w, ToplevelState::Maximized, true, Some(area));
            }
        }
    }

    /// Drops all maximize/fullscreen bookkeeping for a closed window.
    pub(crate) fn forget_window_state(&mut self, w: &Window) {
        self.restore_geometry.retain(|(r, _)| r != w);
        if self.fullscreen.as_ref() == Some(w) {
            self.fullscreen = None;
        }
    }
}
