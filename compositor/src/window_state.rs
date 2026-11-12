//! Maximize and fullscreen.
//!
//! Maximized windows fill the area panels don't reserve (below the bar).
//! A fullscreen window covers the whole output, bar included; while one is
//! showing, the renderer draws only it plus overlay-layer surfaces (e.g. the
//! screenshot overlay), which is also the cheapest frame to compose.
//! Both remember the floating geometry and restore it. Each output has its
//! own: a window fills the output it's on (or the one asked for), and a
//! fullscreen window on one output leaves the others as they were.

use smithay::{
    desktop::Window,
    output::Output,
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as ToplevelState,
    utils::{Logical, Rectangle},
};

use crate::state::WhiteRose;

impl WhiteRose {
    /// The fullscreen window showing on `output` (visible workspace), if any.
    pub fn fullscreen_on(&self, output: &Output) -> Option<&Window> {
        self.fullscreen
            .iter()
            .find(|w| self.space.element_location(w).is_some() && self.output_of(w).as_ref() == Some(output))
    }

    fn output_rect(&self, o: &Output) -> Rectangle<i32, Logical> {
        self.space.output_geometry(o).unwrap_or_default()
    }

    /// Remembers which output `w` fills from now on.
    fn fill(&mut self, w: &Window, o: Output) {
        self.filling.retain(|(f, _)| f != w);
        self.filling.push((w.clone(), o));
    }

    fn remember_floating(&mut self, w: &Window) {
        if self.restore_geometry.iter().any(|(r, _)| r == w) {
            return; // already maximized/fullscreen: keep the original floating geometry
        }
        let loc = self.space.element_location(w).unwrap_or_default();
        self.restore_geometry.push((w.clone(), Rectangle::new(loc, w.geometry().size)));
    }

    pub(crate) fn has_state(w: &Window, state: ToplevelState) -> bool {
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
        let Some(o) = self.output_of(w) else { return };
        self.remember_floating(w);
        let area = self.usable_area_on(&o);
        self.fill(w, o);
        self.apply(w, ToplevelState::Maximized, true, Some(area));
    }

    pub fn unmaximize(&mut self, w: &Window) {
        if !Self::has_state(w, ToplevelState::Maximized) {
            return;
        }
        let rect = if Self::has_state(w, ToplevelState::Fullscreen) {
            None // still fullscreen; only clear the flag
        } else {
            self.filling.retain(|(f, _)| f != w);
            self.take_floating(w)
        };
        self.apply(w, ToplevelState::Maximized, false, rect);
        // Back into its tile, if it's tiled.
        self.retile();
    }

    pub fn fullscreen(&mut self, w: &Window) {
        self.fullscreen_to(w, None);
    }

    /// Fullscreen on `target`, or the output the window is on.
    pub fn fullscreen_to(&mut self, w: &Window, target: Option<Output>) {
        let Some(o) = target.filter(|o| self.space.output_geometry(o).is_some()).or_else(|| self.output_of(w)) else {
            return;
        };
        let already = Self::has_state(w, ToplevelState::Fullscreen);
        if already && self.output_of(w).as_ref() == Some(&o) {
            return;
        }
        // One fullscreen window per output (of those showing: a window a
        // rule opens on another workspace leaves this one be).
        let showing = self.workspaces.workspace_of(w) == Some(self.workspaces.active());
        if let Some(prev) = self.fullscreen_on(&o).filter(|p| showing && *p != w).cloned() {
            self.unfullscreen(&prev);
        }
        self.remember_floating(w);
        let rect = self.output_rect(&o);
        self.fill(w, o);
        self.apply(w, ToplevelState::Fullscreen, true, Some(rect));
        if !self.fullscreen.contains(w) {
            self.fullscreen.push(w.clone());
        }
        self.space.raise_element(w, true);
        // (apply() synced before the list had it.)
        self.island_sync();
    }

    pub fn unfullscreen(&mut self, w: &Window) {
        if !Self::has_state(w, ToplevelState::Fullscreen) {
            return;
        }
        self.fullscreen.retain(|f| f != w);
        let rect = if Self::has_state(w, ToplevelState::Maximized) {
            // Back to maximized (not floating), on the same output.
            self.output_of(w).map(|o| self.usable_area_on(&o))
        } else {
            self.filling.retain(|(f, _)| f != w);
            self.take_floating(w)
        };
        self.apply(w, ToplevelState::Fullscreen, false, rect);
        self.retile();
        self.island_sync();
    }

    pub fn toggle_fullscreen(&mut self, w: &Window) {
        if Self::has_state(w, ToplevelState::Fullscreen) {
            self.unfullscreen(w);
        } else {
            self.fullscreen(w);
        }
    }

    /// Re-fits maximized and fullscreen windows, and the tiles, after the output or the
    /// panels' reserved space changed (bar appeared, output resized).
    /// Cheap when nothing changed: the bar commits often (clock, EQ), so this
    /// only reconfigures windows when the geometry really moved.
    pub fn refit_windows(&mut self) {
        let outputs: Vec<Output> = self.space.outputs().cloned().collect();
        let fitted: Vec<_> = outputs.iter().map(|o| (self.output_rect(o), self.usable_area_on(o))).collect();
        if self.fitted_to == fitted {
            return;
        }
        self.fitted_to = fitted;
        let windows: Vec<Window> = self.space.elements().cloned().collect();
        for w in windows {
            let fs = Self::has_state(&w, ToplevelState::Fullscreen);
            if !fs && !Self::has_state(&w, ToplevelState::Maximized) {
                continue;
            }
            // Its output may be gone: then it moves to the one it overlaps.
            let Some(o) = self.output_of(&w) else { continue };
            self.fill(&w, o.clone());
            let rect = if fs { self.output_rect(&o) } else { self.usable_area_on(&o) };
            self.apply(&w, if fs { ToplevelState::Fullscreen } else { ToplevelState::Maximized }, true, Some(rect));
        }
        self.retile();
    }

    /// Drops all maximize/fullscreen bookkeeping for a closed window.
    pub(crate) fn forget_window_state(&mut self, w: &Window) {
        self.restore_geometry.retain(|(r, _)| r != w);
        self.fullscreen.retain(|f| f != w);
        self.filling.retain(|(f, _)| f != w);
    }
}
