//! Applying window rules (`rules.rs`) when a window opens.
//!
//! A window's app id and title aren't known when it's created (the client
//! sets them before its first commit), so rules run on its first commit
//! (`window_committed`), before it's tiled and before it has drawn: the app
//! draws its first frame where and how big the rules say.
//! They run once; editing the rules doesn't move open windows. What they
//! say about its look (opacity, always on top) stays with the window for
//! its life.

use std::cell::RefCell;

use serde_json::Value;
use smithay::{desktop::Window, output::Output, utils::Point};

use crate::{
    layout::{Layout, Rect},
    outputs,
    rules::{Actions, Rules},
    state::WhiteRose,
};

/// A window's look from its rules, kept in its user data for its life.
#[derive(Debug, Clone, Copy)]
struct Look {
    /// (focused, unfocused).
    opacity: Option<(f32, f32)>,
    on_top: bool,
}

/// A rule's position still to apply: it needs the window's size, which is
/// known once it has drawn. Kept in the window's user data.
struct Placing(RefCell<Option<(Output, Actions)>>);

impl WhiteRose {
    /// Takes the `windows.rules` setting. Nothing to redo: rules apply as
    /// windows open.
    pub fn rules_setting(&mut self, key: &str, value: &Value) {
        if key == "windows.rules" {
            self.rules = Rules::parse(value);
        }
    }

    /// What the rules say for a window on its first commit: monitor,
    /// float, size, maximized, fullscreen and workspace now; position once
    /// it has drawn (`place_by_rule`).
    pub(crate) fn apply_rules(&mut self, w: &Window) {
        let a = self.rules_for(w);
        let active = self.workspaces.active();
        let ws = a.workspace.map(|n| usize::from(n) - 1).filter(|&n| n < self.workspaces.count()).unwrap_or(active);

        let monitor =
            a.monitor.as_deref().and_then(|id| self.space.outputs().find(|o| outputs::info(o).id == id).cloned());
        if let Some(o) = &monitor {
            self.open_on(w, o);
        }
        match a.float {
            Some(true) if !self.tiling.floating.contains(w) => self.tiling.floating.push(w.clone()),
            Some(false) => self.tiling.floating.retain(|f| f != w),
            _ => {}
        }
        // It belongs to its workspace before it's maximized or fullscreen,
        // so a fullscreen window that's showing isn't put out for it.
        let elsewhere = ws != active && self.workspaces.move_to(w, ws);

        let maximized = a.maximized == Some(true);
        let fullscreen = a.fullscreen == Some(true);
        if let Some(o) = self.output_of(w).filter(|o| self.floats_on(w, ws, o)) {
            let area = self.usable_area_on(&o);
            let area = Rect::new(area.loc.x, area.loc.y, area.size.w, area.size.h);
            if let (Some((width, height)), Some(t)) = (a.size_in(area), w.toplevel()) {
                t.with_pending_state(|s| s.size = Some((width, height).into()));
            }
            if a.position.is_some() && !maximized && !fullscreen {
                w.user_data().insert_if_missing(|| Placing(RefCell::new(None)));
                if let Some(p) = w.user_data().get::<Placing>() {
                    *p.0.borrow_mut() = Some((o, a.clone()));
                }
            }
        }
        if maximized {
            self.maximize(w);
        }
        if fullscreen {
            self.fullscreen_to(w, monitor);
        } else if !elsewhere {
            // Opening on a fullscreen window's output: it must be seen.
            if let Some(fs) = self.output_of(w).and_then(|o| self.fullscreen_on(&o).cloned()).filter(|f| f != w) {
                self.unfullscreen(&fs);
            }
        }

        // On another workspace it opens quietly: that workspace doesn't
        // show, and focus goes back to where it was.
        if elsewhere {
            self.hide(w);
            w.set_activated(false);
            let next = self.workspaces.focused().cloned();
            self.focus_window(next);
        }
        // Its look, for its life. Above the others now if it's on top (it
        // was raised when it was created, before the rules were known).
        if a.opacity.is_some() || a.on_top == Some(true) {
            w.user_data().insert_if_missing(|| Look { opacity: a.opacity, on_top: a.on_top == Some(true) });
            self.restack();
        }

        // Its first configure went out when it was created (focusing it);
        // this one carries what the rules changed, before it draws.
        if let Some(t) = w.toplevel() {
            t.send_pending_configure();
        }
    }

    /// How opaque to draw `w` (1: solid), by its rules and whether it has
    /// the keyboard.
    pub fn window_alpha(&self, w: &Window) -> f32 {
        let Some((focused, unfocused)) = w.user_data().get::<Look>().and_then(|l| l.opacity) else { return 1.0 };
        let has_focus = self.keyboard_focus().zip(w.toplevel()).is_some_and(|(f, t)| &f == t.wl_surface());
        if has_focus {
            focused
        } else {
            unfocused
        }
    }

    /// Whether a rule keeps `w` above the other windows.
    pub fn on_top(w: &Window) -> bool {
        w.user_data().get::<Look>().is_some_and(|l| l.on_top)
    }

    fn rules_for(&self, w: &Window) -> Actions {
        if self.rules.is_empty() {
            return Actions::default();
        }
        let (app, title) = crate::ipc::toplevel_ids(w);
        let a = self.rules.resolve(app.as_deref().unwrap_or_default(), title.as_deref().unwrap_or_default());
        if !a.is_empty() {
            tracing::info!(app = app.as_deref().unwrap_or_default(), "window rules apply");
        }
        a
    }

    /// Whether `w` will float on workspace `ws` of `o`, so a size and a
    /// position mean something.
    fn floats_on(&self, w: &Window, ws: usize, o: &Output) -> bool {
        !self.tileable(w) || self.layout_of(ws, o) == Layout::Floating
    }

    /// Moves a window that hasn't drawn yet to `o`, at the same place in its
    /// usable area as on the output it opened on (the cascade).
    fn open_on(&mut self, w: &Window, o: &Output) {
        let (Some(from), Some(at)) = (self.output_of(w), self.space.element_location(w)) else { return };
        if from == *o {
            return;
        }
        let at = self.usable_area_on(o).loc + (at - self.usable_area_on(&from).loc);
        self.space.map_element(w.clone(), at, false);
    }

    /// On a later commit: once the window has drawn, puts it where its rule
    /// said (its size is known now). Once only.
    pub(crate) fn place_by_rule(&mut self, w: &Window) {
        let Some(p) = w.user_data().get::<Placing>() else { return };
        let size = w.geometry().size;
        if size.w <= 0 || size.h <= 0 {
            return;
        }
        let Some((o, a)) = p.0.borrow_mut().take() else { return };
        // Tiled or filling a screen since: nothing to place.
        if self.is_tiled(w) || Self::maximized_or_fullscreen(w) || self.space.output_geometry(&o).is_none() {
            return;
        }
        let area = self.usable_area_on(&o);
        let area = Rect::new(area.loc.x, area.loc.y, area.size.w, area.size.h);
        let Some((x, y)) = a.position_in(area, (size.w, size.h)) else { return };
        let at = Point::from((x, y));
        if self.space.element_location(w).is_some() {
            self.space.map_element(w.clone(), at, false);
        } else if let Some(h) = self.hidden_at.iter_mut().find(|(h, _)| h == w) {
            h.1 = at;
        }
    }
}
