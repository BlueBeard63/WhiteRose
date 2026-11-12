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
//!
//! Resizing a snapped window (its border, SUPER+right-drag, or the gap
//! between snapped windows) moves the line it shares with the snapped
//! windows beside it, so they resize together.
//!
//! Holding SHIFT while dragging shows the monitor's zone grid
//! (`windows.snap.zones`, else two thirds and a third) instead, and the
//! window drops into the zone under the pointer.
//!
//! The picker shows the layouts (halves, thirds, quarters, two thirds and a
//! third, the monitor's own grid) as small diagrams; a cell puts the window
//! there. It opens with SUPER+Z (the focused window), when a drag reaches
//! the top edge's middle, or when the pointer rests on the maximize button
//! of a WhiteRose app (the kit asks for it). The compositor places it and
//! finds the cell under the pointer, even while a drag holds the pointer;
//! the shell only draws it (`snap_picker` event).

use smithay::{
    backend::renderer::{element::Id, utils::CommitCounter},
    desktop::Window,
    input::pointer::{Focus, GrabStartData},
    output::Output,
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as ToplevelState,
    utils::{Logical, Point, Rectangle, Serial},
};

use crate::{
    grabs::SnapResizeGrab,
    layout::{
        snap::{self, EdgeDrag, KeyState, KeyStep, Open, PickerGeometry, PickerLayout, Target, Zone},
        Axis, Dir, Layout, Rect,
    },
    outputs,
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
    /// The floating window being dragged, and where the pointer is.
    pub dragging: Option<(Window, Point<f64, Logical>)>,
    /// What the window being dragged snaps to if it's dropped now.
    pub drag_target: Option<(Output, Target)>,
    /// The zone grid shown while SHIFT is held (global coordinates).
    pub grid: Vec<(Rectangle<i32, Logical>, Id)>,
    /// Bumped when the grid changes, so the renderer redraws it.
    pub grid_commit: CommitCounter,
    /// The layout picker, while it's open.
    pub picker: Option<OpenPicker>,
}

/// How the picker was opened, which decides how it closes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerMode {
    /// SUPER+Z: the arrows, Enter and Escape work it; a click outside
    /// closes it.
    Key,
    /// A drag reached the top edge's middle: dropping on a cell snaps
    /// there; moving away closes it.
    Drag,
    /// The pointer rested on a WhiteRose app's maximize button: moving well
    /// away closes it.
    Hover,
}

#[derive(Debug)]
pub struct OpenPicker {
    pub window: Window,
    pub output: Output,
    pub mode: PickerMode,
    /// Its top-left corner (global).
    pub at: Point<i32, Logical>,
    /// A card under the pointer, not hanging from the bar like the island.
    pub card: bool,
    pub layouts: Vec<PickerLayout>,
    pub geometry: PickerGeometry,
    /// The (layout, cell) under the pointer or chosen with the arrows.
    pub hovered: Option<(usize, usize)>,
}

impl OpenPicker {
    pub fn rect(&self) -> Rectangle<i32, Logical> {
        Rectangle::new(self.at, (self.geometry.width, self.geometry.height).into())
    }

    fn hit(&self, pos: Point<f64, Logical>) -> Option<(usize, usize)> {
        self.geometry.hit(&self.layouts, pos.x - self.at.x as f64, pos.y - self.at.y as f64)
    }

    /// Whether `pos` is on it, or within `margin` of it.
    fn near(&self, pos: Point<f64, Logical>, margin: i32) -> bool {
        let r = self.rect();
        let grown = Rectangle::<i32, Logical>::new(
            (r.loc.x - margin, r.loc.y - margin).into(),
            (r.size.w + 2 * margin, r.size.h + 2 * margin).into(),
        );
        grown.to_f64().contains(pos)
    }
}

/// How far (px) the pointer can stray from a hover-opened picker (it opens
/// a little below the button) or a drag's picker before it closes.
const HOVER_REACH: i32 = 48;
const DRAG_REACH: i32 = 24;

/// The accent rose (#E8A3AE) at 12%, premultiplied: the zones not under
/// the pointer (the one under it gets the drop preview on top).
pub const GRID_COLOR: [f32; 4] = [0.910 * 0.12, 0.639 * 0.12, 0.682 * 0.12, 0.12];

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
        if self.snap.picker.as_ref().is_some_and(|p| p.window == *w) {
            self.close_picker();
        }
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

    /// The zone grid of output `o`: its own, else two thirds and a third.
    fn grid_of(&self, o: &Output) -> Vec<Zone> {
        let id = outputs::info(o).id;
        self.tiling.settings.snap_zones.get(&id).cloned().unwrap_or_else(snap::default_grid)
    }

    /// Shows `rects` as the zone grid (none hides it).
    fn show_grid(&mut self, rects: Vec<Rectangle<i32, Logical>>) {
        if self.snap.grid.iter().map(|(r, _)| *r).eq(rects.iter().copied()) {
            return;
        }
        self.snap.grid = rects.into_iter().map(|r| (r, Id::new())).collect();
        self.snap.grid_commit.increment();
        self.schedule_redraw();
    }

    fn shift_held(&self) -> bool {
        self.seat.get_keyboard().is_some_and(|k| k.modifier_state().shift)
    }

    /// The dragged window moved: shows what it snaps to, if anything.
    pub fn snap_drag_motion(&mut self, w: &Window, pos: Point<f64, Logical>) {
        self.snap.dragging = Some((w.clone(), pos));
        if self.shift_held() {
            self.zone_drag_motion(w, pos);
            return;
        }
        self.show_grid(Vec::new());
        // On the picker a drag opened: the cell under the pointer.
        if let Some(p) = self.snap.picker.as_ref().filter(|p| p.mode == PickerMode::Drag) {
            if p.near(pos, DRAG_REACH) {
                let hovered = p.hit(pos);
                let target = hovered.map(|(l, c)| (p.output.clone(), Target::Zone(p.layouts[l].cells[c])));
                self.snap.drag_target = target;
                self.set_picker_hovered(hovered);
                return;
            }
        }
        let target = self.snap_target_at(w, pos);
        if let Some((o, Target::Picker)) = target {
            if self.snap.picker.is_none() {
                self.open_picker(w, o, PickerMode::Drag, None);
            }
            self.snap.drag_target = None;
            self.set_preview(None);
            return;
        }
        if self.snap.picker.as_ref().is_some_and(|p| p.mode == PickerMode::Drag) {
            self.close_picker();
        }
        let preview = target.as_ref().map(|(o, t)| match t {
            Target::Zone(z) => self.zone_rect(z, o),
            Target::Maximize | Target::Picker => self.usable_area_on(o),
        });
        self.snap.drag_target = target;
        self.set_preview(preview);
    }

    /// With SHIFT held: the zone grid of the output under the pointer, and
    /// the zone under it.
    fn zone_drag_motion(&mut self, w: &Window, pos: Point<f64, Logical>) {
        let Some(o) = self.output_at(pos).filter(|o| self.can_snap(w, o)) else {
            self.snap.drag_target = None;
            self.show_grid(Vec::new());
            self.set_preview(None);
            return;
        };
        let grid = self.grid_of(&o);
        let rects: Vec<_> = grid.iter().map(|z| self.zone_rect(z, &o)).collect();
        let under = snap::zone_at(&grid, to_rect(self.usable_area_on(&o)), pos.x, pos.y);
        self.set_preview(under.map(|i| rects[i]));
        self.snap.drag_target = under.map(|i| (o, Target::Zone(grid[i])));
        self.show_grid(rects);
    }

    /// SHIFT went down or up: the drag shows or hides the zones.
    pub fn snap_drag_refresh(&mut self) {
        if let Some((w, pos)) = self.snap.dragging.clone() {
            self.snap_drag_motion(&w, pos);
        }
    }

    /// The window dragged from `from` was dropped: snaps it to what was
    /// shown.
    pub fn snap_drop(&mut self, w: &Window, from: Point<i32, Logical>) {
        self.snap.dragging = None;
        self.show_grid(Vec::new());
        if self.snap.picker.as_ref().is_some_and(|p| p.mode == PickerMode::Drag) {
            self.close_picker();
        }
        let Some((o, target)) = self.snap.drag_target.take() else { return };
        self.set_preview(None);
        match target {
            Target::Zone(z) => self.snap_window_from(w, o, z, Some(from)),
            Target::Maximize | Target::Picker => self.maximize_on(w, o, from),
        }
    }

    /// The drag ended without a drop (the grab was taken away).
    pub fn snap_drag_end(&mut self) {
        self.snap.dragging = None;
        self.show_grid(Vec::new());
        if self.snap.picker.as_ref().is_some_and(|p| p.mode == PickerMode::Drag) {
            self.close_picker();
        }
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

/// Resizing snapped windows together (grabs/tile_grab.rs).
impl WhiteRose {
    /// The snapped windows showing on `o`, with their zones.
    fn snapped_on(&self, o: &Output) -> Vec<(Window, Zone)> {
        self.snap
            .snapped
            .iter()
            .filter(|s| s.output == *o && self.space.element_location(&s.window).is_some())
            .map(|s| (s.window.clone(), s.zone))
            .collect()
    }

    /// Starts moving the edges `edges` of snapped window `w`, and the
    /// snapped windows' edges on the same lines. Edges on the area's border
    /// don't move; if none can, nothing happens.
    pub fn start_snap_resize(
        &mut self,
        w: &Window,
        edges: Vec<Dir>,
        start_data: GrabStartData<WhiteRose>,
        serial: Serial,
    ) {
        let Some(o) = self.snap_of(w).map(|s| s.output.clone()) else { return };
        let snapped = self.snapped_on(&o);
        let drags: Vec<(EdgeDrag<Window>, f64)> = edges
            .into_iter()
            .filter_map(|d| EdgeDrag::new(&snapped, w, d))
            .map(|d| {
                let line = d.line;
                (d, line)
            })
            .collect();
        if drags.is_empty() {
            return;
        }
        let Some(pointer) = self.seat.get_pointer() else { return };
        let grab = SnapResizeGrab { start_data, output: o, drags };
        pointer.set_grab(self, grab, serial, Focus::Clear);
    }

    /// Moves each dragged line by the pointer's `delta` from where it
    /// started, and refits the snapped windows on it.
    pub fn resize_snapped(&mut self, o: &Output, drags: &mut [(EdgeDrag<Window>, f64)], delta: Point<f64, Logical>) {
        let c = self.usable_area_on(o);
        let c = to_rect(c).shrink(self.tiling.settings.gaps.outer.max(0));
        let mut zones = self.snapped_on(o);
        for (d, start) in drags.iter_mut() {
            let moved = match d.axis {
                Axis::Horizontal => delta.x / c.w.max(1) as f64,
                Axis::Vertical => delta.y / c.h.max(1) as f64,
            };
            d.drag(&mut zones, *start + moved);
        }
        for (w, zone) in zones {
            if let Some(s) = self.snap.snapped.iter_mut().find(|s| s.window == w) {
                if !s.zone.same_as(&zone) {
                    s.zone = zone;
                    self.place_snapped(&w);
                }
            }
        }
        self.schedule_redraw();
    }

    /// The snapped window edges at `pos`, when it's in the gap beside a
    /// snapped window (on no window): that window, and which of its edges.
    pub fn snap_gap_at(&self, pos: Point<f64, Logical>) -> Option<(Window, Vec<Dir>)> {
        let o = self.output_at(pos)?;
        let p = pos.to_i32_round::<i32>();
        // Within half the gap of a window (and a few pixels more, so thin
        // gaps can be caught too).
        let reach = self.tiling.settings.gaps.inner / 2 + 4;
        let snapped = self.snapped_on(&o);
        self.snap.snapped.iter().filter(|s| s.output == o && !Self::maximized_or_fullscreen(&s.window)).find_map(|s| {
            self.space.element_location(&s.window)?;
            let r = to_rect(self.zone_rect(&s.zone, &o));
            let grown = Rect::new(r.x - reach, r.y - reach, r.w + 2 * reach, r.h + 2 * reach);
            if !to_rectangle(grown).contains(p) || to_rectangle(r).contains(p) {
                return None;
            }
            let edges: Vec<Dir> = [
                (p.x < r.x, Dir::Left),
                (p.x >= r.right(), Dir::Right),
                (p.y < r.y, Dir::Up),
                (p.y >= r.bottom(), Dir::Down),
            ]
            .into_iter()
            // Only edges that can move: not the area's border.
            .filter(|(beyond, d)| *beyond && EdgeDrag::new(&snapped, &s.window, *d).is_some())
            .map(|(_, d)| d)
            .collect();
            (!edges.is_empty()).then(|| (s.window.clone(), edges))
        })
    }
}

/// The layout picker.
impl WhiteRose {
    /// SUPER+Z (the focused window), or a WhiteRose app's maximize button
    /// (`under_pointer`: the window under the pointer, the picker just
    /// below it). SUPER+Z again closes it.
    pub fn snap_picker(&mut self, under_pointer: bool) {
        if self.snap.picker.is_some() {
            self.close_picker();
            if !under_pointer {
                return;
            }
        }
        let pointer = self.seat.get_pointer().map(|p| p.current_location()).unwrap_or_default();
        let (w, mode, anchor) = if under_pointer {
            let Some((w, _)) = self.window_under(pointer) else { return };
            (w.clone(), PickerMode::Hover, Some(pointer))
        } else {
            let Some(w) = self.focused_window() else { return };
            (w, PickerMode::Key, None)
        };
        let Some(o) = (if under_pointer { self.output_at(pointer) } else { self.output_of(&w) }) else { return };
        if !self.can_snap(&w, &o) {
            return;
        }
        self.open_picker(&w, o, mode, anchor);
    }

    /// Opens the picker for `w` on `o`: hanging from the top of the usable
    /// area in the middle, or as a card just below `anchor`.
    fn open_picker(&mut self, w: &Window, o: Output, mode: PickerMode, anchor: Option<Point<f64, Logical>>) {
        let id = outputs::info(&o).id;
        let grid = self.tiling.settings.snap_zones.get(&id).cloned();
        let layouts = snap::picker_layouts(grid.as_deref());
        let geometry = PickerGeometry::new(&layouts);
        let area = self.usable_area_on(&o);
        let (gw, gh) = (geometry.width, geometry.height);
        let at = match anchor {
            Some(p) => {
                let margin = 8;
                let max_x = (area.loc.x + area.size.w - gw - margin).max(area.loc.x);
                let max_y = (area.loc.y + area.size.h - gh - margin).max(area.loc.y);
                let x = (p.x as i32 - gw / 2).clamp(area.loc.x + margin, max_x);
                let y = (p.y as i32 + 16).clamp(area.loc.y + margin, max_y);
                Point::from((x, y))
            }
            None => Point::from((area.loc.x + (area.size.w - gw) / 2, area.loc.y)),
        };
        let hovered = (mode == PickerMode::Key).then_some((0, 0));
        let card = anchor.is_some();
        self.snap.picker =
            Some(OpenPicker { window: w.clone(), output: o, mode, at, card, layouts, geometry, hovered });
        self.picker_changed();
    }

    pub fn close_picker(&mut self) {
        if self.snap.picker.take().is_some() {
            self.set_preview(None);
            self.picker_sync();
        }
    }

    fn set_picker_hovered(&mut self, hovered: Option<(usize, usize)>) {
        let Some(p) = self.snap.picker.as_mut() else { return };
        if p.hovered != hovered {
            p.hovered = hovered;
            self.picker_changed();
        }
    }

    /// The hovered cell's zone shows as the drop preview; the shell redraws.
    fn picker_changed(&mut self) {
        let preview = self.snap.picker.as_ref().and_then(|p| {
            let (l, c) = p.hovered?;
            Some(self.zone_rect(&p.layouts[l].cells[c], &p.output))
        });
        self.set_preview(preview);
        self.picker_sync();
    }

    /// Snaps the picker's window into the hovered cell, and closes it.
    fn picker_choose(&mut self) {
        let Some(p) = self.snap.picker.as_ref() else { return };
        let chosen = p.hovered.map(|(l, c)| (p.window.clone(), p.output.clone(), p.layouts[l].cells[c]));
        self.close_picker();
        if let Some((w, o, zone)) = chosen {
            self.snap_window(&w, o, zone);
        }
    }

    /// The pointer moved (not in a drag): the cell under it is hovered; a
    /// hover-opened picker closes once the pointer is well away.
    pub fn picker_pointer(&mut self, pos: Point<f64, Logical>) {
        let Some(p) = self.snap.picker.as_ref().filter(|p| p.mode != PickerMode::Drag) else { return };
        if p.near(pos, 0) {
            let hovered = p.hit(pos);
            if hovered.is_some() || p.mode == PickerMode::Hover {
                self.set_picker_hovered(hovered);
            }
        } else if p.mode == PickerMode::Hover && !p.near(pos, HOVER_REACH) {
            self.close_picker();
        }
    }

    /// A button was pressed at `pos` (not in a drag). On the picker: a cell
    /// chooses it, and the click is the picker's (true). Elsewhere it
    /// closes the picker and the click goes on as usual.
    pub fn picker_click(&mut self, pos: Point<f64, Logical>) -> bool {
        let Some(p) = self.snap.picker.as_ref().filter(|p| p.mode != PickerMode::Drag) else { return false };
        if !p.near(pos, 0) {
            self.close_picker();
            return false;
        }
        let hovered = p.hit(pos);
        if hovered.is_some() {
            self.set_picker_hovered(hovered);
            self.picker_choose();
        }
        true
    }

    /// A key pressed while the picker is open (not in a drag): arrows move
    /// between cells, Enter chooses, Escape closes. True if it was the
    /// picker's; any other key closes it and works as usual.
    pub fn picker_key(&mut self, sym: smithay::input::keyboard::Keysym) -> bool {
        use smithay::input::keyboard::keysyms as k;
        let Some(p) = self.snap.picker.as_ref().filter(|p| p.mode != PickerMode::Drag) else { return false };
        let dir = match sym.raw() {
            k::KEY_Left => Some(Dir::Left),
            k::KEY_Right => Some(Dir::Right),
            k::KEY_Up => Some(Dir::Up),
            k::KEY_Down => Some(Dir::Down),
            _ => None,
        };
        if let Some(d) = dir {
            let next = snap::picker_step(&p.geometry, p.hovered, d);
            self.set_picker_hovered(next);
            return true;
        }
        match sym.raw() {
            k::KEY_Return | k::KEY_KP_Enter => {
                self.picker_choose();
                true
            }
            k::KEY_Escape => {
                self.close_picker();
                true
            }
            _ if sym.is_modifier_key() => false,
            _ => {
                self.close_picker();
                false
            }
        }
    }

    /// What the shell draws.
    pub fn picker_event(&self) -> crate::ipc::Event {
        let picker = self.snap.picker.as_ref().map(|p| {
            let og = self.space.output_geometry(&p.output).unwrap_or_default();
            let r = |r: &Rect| [r.x, r.y, r.w, r.h];
            crate::ipc::PickerView {
                output: p.output.name(),
                x: p.at.x - og.loc.x,
                y: p.at.y - og.loc.y,
                width: p.geometry.width,
                height: p.geometry.height,
                card: p.card,
                title: crate::ipc::toplevel_ids(&p.window).1.unwrap_or_default(),
                layouts: p
                    .layouts
                    .iter()
                    .zip(&p.geometry.tiles)
                    .map(|(l, (tile, cells))| crate::ipc::PickerLayoutView {
                        id: l.id.to_string(),
                        rect: r(tile),
                        cells: cells.iter().map(r).collect(),
                    })
                    .collect(),
                hovered: p.hovered.map(|(l, c)| [l, c]),
            }
        });
        crate::ipc::Event::SnapPicker { picker }
    }

    fn picker_sync(&mut self) {
        let event = self.picker_event();
        if let Some(ipc) = self.ipc.as_mut() {
            ipc.update(event);
        }
        self.schedule_redraw();
    }
}
