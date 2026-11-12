//! Snapping floating windows (on floating workspaces): drag one to a side
//! edge for a half or to a corner for a quarter, to the top to maximize;
//! SUPER+SHIFT+arrows step it between snaps as Win+arrows do.
//!
//! A snapped window keeps its zone as fractions of its output's usable area
//! (`layout::snap`), so it's refitted when the bar or the outputs change,
//! and it remembers its own size from before it snapped: dragging it away
//! or snapping it back gives that size back. Maximize, fullscreen and
//! minimise keep the snap; moving it to another workspace, or the workspace
//! starting to tile, ends it.

use smithay::{
    desktop::Window,
    output::Output,
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as ToplevelState,
    utils::{Logical, Point, Rectangle},
};

use crate::{
    layout::{
        snap::{self, KeyState, KeyStep, Open, Target, Zone},
        Dir, Layout, Rect,
    },
    state::WhiteRose,
    tiling::TILED,
};

/// A snapped window.
#[derive(Debug, Clone)]
pub struct Snapped {
    pub window: Window,
    pub zone: Zone,
    pub output: Output,
    /// Where it was, and its size, before it snapped.
    pub before: Rectangle<i32, Logical>,
}

#[derive(Debug, Default)]
pub struct SnapState {
    pub snapped: Vec<Snapped>,
    /// What the window being dragged snaps to if it's dropped now.
    pub drag_target: Option<(Output, Target)>,
}

fn to_rect(r: Rectangle<i32, Logical>) -> Rect {
    Rect::new(r.loc.x, r.loc.y, r.size.w, r.size.h)
}

fn to_rectangle(r: Rect) -> Rectangle<i32, Logical> {
    Rectangle::new((r.x, r.y).into(), (r.w, r.h).into())
}

impl WhiteRose {
    pub fn snap_of(&self, w: &Window) -> Option<&Snapped> {
        self.snap.snapped.iter().find(|s| s.window == *w)
    }

    /// Whether `w` can snap on output `o`: snapping is on, the visible
    /// workspace floats there, and the window isn't a tile, fullscreen or
    /// fixed-size.
    fn can_snap(&self, w: &Window, o: &Output) -> bool {
        self.tiling.settings.snap
            && self.layout_of(self.workspaces.active(), o) == Layout::Floating
            && !self.is_tiled(w)
            && !Self::has_state(w, ToplevelState::Fullscreen)
            && !Self::fixed_size(w)
    }

    /// Where a window is and the size it was last told (or chose).
    fn current_rect(&self, w: &Window) -> Rectangle<i32, Logical> {
        let loc = self.space.element_location(w).unwrap_or_default();
        let told = w.toplevel().and_then(|t| t.with_pending_state(|s| s.size)).filter(|s| s.w > 0 && s.h > 0);
        Rectangle::new(loc, told.unwrap_or_else(|| w.geometry().size))
    }

    /// The rectangle a zone gets on `o`, with the tiles' gaps.
    fn zone_rect(&self, zone: &Zone, o: &Output) -> Rectangle<i32, Logical> {
        to_rectangle(zone.rect(to_rect(self.usable_area_on(o)), &self.tiling.settings.gaps))
    }

    /// Snaps `w` into `zone` on `o`. A maximized window stops being
    /// maximized; a window already snapped keeps its size from before.
    pub fn snap_window(&mut self, w: &Window, o: Output, zone: Zone) {
        self.snap_window_from(w, o, zone, None);
    }

    /// [`Self::snap_window`] for a window dragged from `from`: that's where
    /// it goes back to, not where the drag left it.
    fn snap_window_from(&mut self, w: &Window, o: Output, zone: Zone, from: Option<Point<i32, Logical>>) {
        let mut before = match self.snap_of(w) {
            Some(s) => s.before,
            None => self
                .restore_geometry
                .iter()
                .find(|(r, _)| r == w)
                .map(|(_, r)| *r)
                .unwrap_or_else(|| self.current_rect(w)),
        };
        if let Some(at) = from {
            before.loc = at;
        }
        self.snap.snapped.retain(|s| s.window != *w);
        self.snap.snapped.push(Snapped { window: w.clone(), zone, output: o, before });
        if Self::has_state(w, ToplevelState::Maximized) {
            // Its floating place is the snap's `before` now.
            self.unmaximize(w);
        }
        self.place_snapped(w);
        self.schedule_redraw();
    }

    /// Gives a snapped window its zone's rectangle, if it isn't there yet.
    /// Maximized, fullscreen and hidden windows wait until they're back.
    fn place_snapped(&mut self, w: &Window) {
        let Some(s) = self.snap_of(w) else { return };
        if Self::maximized_or_fullscreen(w) || self.space.element_location(w).is_none() {
            return;
        }
        let rect = self.zone_rect(&s.zone, &s.output);
        let Some(t) = w.toplevel() else { return };
        let changed = t.with_pending_state(|p| {
            let changed = p.size != Some(rect.size) || !TILED.iter().all(|st| p.states.contains(*st));
            p.size = Some(rect.size);
            for st in TILED {
                p.states.set(st);
            }
            changed
        });
        if changed {
            t.send_pending_configure();
        }
        if self.space.element_location(w) != Some(rect.loc) {
            self.space.map_element(w.clone(), rect.loc, false);
        }
    }

    /// Refits every snapped window on screen (part of `retile`): after the
    /// bar, the outputs or the gaps changed, or a window came back. A
    /// window whose output is gone moves to the one it's on now.
    pub(crate) fn refit_snapped(&mut self) {
        let outputs: Vec<Output> = self.space.outputs().cloned().collect();
        for i in 0..self.snap.snapped.len() {
            if !outputs.contains(&self.snap.snapped[i].output) {
                let w = self.snap.snapped[i].window.clone();
                if let Some(o) = self.output_of(&w).or_else(|| outputs.first().cloned()) {
                    self.snap.snapped[i].output = o;
                }
            }
        }
        let windows: Vec<Window> = self.snap.snapped.iter().map(|s| s.window.clone()).collect();
        for w in windows {
            self.place_snapped(&w);
        }
    }

    /// Ends a window's snap where it is (resized by hand, moved to another
    /// workspace, tiled). Returns what it was.
    pub(crate) fn end_snap(&mut self, w: &Window) -> Option<Snapped> {
        let i = self.snap.snapped.iter().position(|s| s.window == *w)?;
        let s = self.snap.snapped.remove(i);
        if let Some(t) = w.toplevel() {
            t.with_pending_state(|p| {
                for st in TILED {
                    p.states.unset(st);
                }
            });
            t.send_pending_configure();
        }
        Some(s)
    }

    /// Ends a snap and gives the window back its size from before, at
    /// `loc` (or where it was before).
    fn unsnap(&mut self, w: &Window, loc: Option<Point<i32, Logical>>) -> Option<Rectangle<i32, Logical>> {
        let s = self.end_snap(w)?;
        let rect = Rectangle::new(loc.unwrap_or(s.before.loc), s.before.size);
        if let Some(t) = w.toplevel() {
            // A window that never chose a size lets the client choose again.
            t.with_pending_state(|p| p.size = Some(rect.size).filter(|sz| sz.w > 0 && sz.h > 0));
            t.send_pending_configure();
        }
        self.space.map_element(w.clone(), rect.loc, false);
        self.schedule_redraw();
        Some(rect)
    }

    /// A closed window's snap.
    pub(crate) fn forget_snap(&mut self, w: &Window) {
        self.snap.snapped.retain(|s| s.window != *w);
    }

    /// A snapped window starts being dragged with the pointer at `pointer`:
    /// it gets its own size back under the pointer, at the same place
    /// across its width. Returns where it is now.
    pub fn unsnap_for_drag(&mut self, w: &Window, pointer: Point<f64, Logical>) -> Option<Point<i32, Logical>> {
        let now = self.snap_of(w).map(|_| self.current_rect(w))?;
        let before = self.snap_of(w)?.before;
        let across = ((pointer.x - now.loc.x as f64) / now.size.w.max(1) as f64).clamp(0.0, 1.0);
        let x = (pointer.x - across * before.size.w as f64).round() as i32;
        let loc = Point::from((x, now.loc.y));
        self.unsnap(w, Some(loc)).map(|r| r.loc)
    }

    /// The sides of output `og` with no other output beyond them, at the
    /// pointer's height (or width).
    fn open_sides(&self, og: Rectangle<i32, Logical>, pos: Point<f64, Logical>) -> Open {
        let beyond = |x: f64, y: f64| self.output_at((x, y).into()).is_some();
        let (l, t) = (og.loc.x as f64, og.loc.y as f64);
        let (r, b) = ((og.loc.x + og.size.w) as f64, (og.loc.y + og.size.h) as f64);
        Open {
            left: !beyond(l - 1.0, pos.y),
            right: !beyond(r, pos.y),
            top: !beyond(pos.x, t - 1.0),
            bottom: !beyond(pos.x, b),
        }
    }

    /// What dragging `w` with the pointer at `pos` snaps to.
    fn snap_target_at(&self, w: &Window, pos: Point<f64, Logical>) -> Option<(Output, Target)> {
        let o = self.output_at(pos)?;
        if !self.can_snap(w, &o) {
            return None;
        }
        let og = self.space.output_geometry(&o)?;
        snap::edge_target(to_rect(og), self.open_sides(og, pos), pos.x, pos.y).map(|t| (o, t))
    }

    /// The dragged window moved: shows what it snaps to, if anything.
    pub fn snap_drag_motion(&mut self, w: &Window, pos: Point<f64, Logical>) {
        let target = self.snap_target_at(w, pos);
        let preview = target.as_ref().map(|(o, t)| match t {
            Target::Zone(z) => self.zone_rect(z, o),
            // (The picker's cells come with the picker; until then, the
            // middle of the top edge maximizes like the rest of it.)
            Target::Maximize | Target::Picker => self.usable_area_on(o),
        });
        self.snap.drag_target = target;
        self.set_preview(preview);
    }

    /// The window dragged from `from` was dropped: snaps it to what was
    /// shown.
    pub fn snap_drop(&mut self, w: &Window, from: Point<i32, Logical>) {
        let Some((o, target)) = self.snap.drag_target.take() else { return };
        self.set_preview(None);
        match target {
            Target::Zone(z) => self.snap_window_from(w, o, z, Some(from)),
            Target::Maximize | Target::Picker => self.maximize_on(w, o, from),
        }
    }

    /// The drag ended without a drop (the grab was taken away).
    pub fn snap_drag_end(&mut self) {
        if self.snap.drag_target.take().is_some() {
            self.set_preview(None);
        }
    }

    /// Maximizes the window dragged from `from` on `o`; restoring puts it
    /// back there at the size it has now (which may still be on its way to
    /// the client).
    fn maximize_on(&mut self, w: &Window, o: Output, from: Point<i32, Logical>) {
        let rect = Rectangle::new(from, self.current_rect(w).size);
        if self.output_of(w).as_ref() != Some(&o) {
            let at = self.usable_area_on(&o).loc;
            self.space.map_element(w.clone(), at, false);
        }
        self.maximize(w);
        if let Some((_, r)) = self.restore_geometry.iter_mut().find(|(r, _)| r == w) {
            *r = rect;
        }
    }

    /// The nearest output beside `o` in direction `dir` (lined up with it).
    fn output_beside(&self, o: &Output, dir: Dir) -> Option<Output> {
        let g = self.space.output_geometry(o)?;
        let (gl, gt, gr, gb) = (g.loc.x, g.loc.y, g.loc.x + g.size.w, g.loc.y + g.size.h);
        self.space
            .outputs()
            .filter(|n| *n != o)
            .filter_map(|n| {
                let c = self.space.output_geometry(n)?;
                let (cl, ct, cr, cb) = (c.loc.x, c.loc.y, c.loc.x + c.size.w, c.loc.y + c.size.h);
                let (distance, overlap) = match dir {
                    Dir::Left => (gl - cr, gb.min(cb) - gt.max(ct)),
                    Dir::Right => (cl - gr, gb.min(cb) - gt.max(ct)),
                    Dir::Up => (gt - cb, gr.min(cr) - gl.max(cl)),
                    Dir::Down => (ct - gb, gr.min(cr) - gl.max(cl)),
                };
                (distance >= 0 && overlap > 0).then_some((distance, n.clone()))
            })
            .min_by_key(|(d, _)| *d)
            .map(|(_, n)| n)
    }

    /// SUPER+SHIFT+arrow on a window that can snap: the next snap (see
    /// `snap::key_step`). False if it can't snap here.
    pub fn snap_key(&mut self, dir: Dir) -> bool {
        let Some(w) = self.focused_window() else { return false };
        let Some(o) = self.output_of(&w) else { return false };
        if !self.can_snap(&w, &o) {
            return false;
        }
        let maximized = Self::has_state(&w, ToplevelState::Maximized);
        let state = match self.snap_of(&w) {
            _ if maximized => KeyState::Maximized,
            Some(s) => KeyState::Snapped(s.zone),
            None => KeyState::Free,
        };
        match snap::key_step(state, dir) {
            KeyStep::Snap(z) => self.snap_window(&w, o, z),
            KeyStep::Maximize => self.maximize(&w),
            KeyStep::Restore if maximized => self.unmaximize(&w),
            KeyStep::Restore => {
                self.unsnap(&w, None);
            }
            KeyStep::NextOutput(d, z) => {
                if let Some(n) = self.output_beside(&o, d) {
                    self.snap_window(&w, n, z);
                }
            }
            KeyStep::Nothing => {}
        }
        true
    }
}
