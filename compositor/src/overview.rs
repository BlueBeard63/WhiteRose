//! The overview, GNOME-style (SUPER+W; Figma "Overview - SUPER + W"): on
//! every monitor, under the bar, the current workspace's windows spread out
//! live so none overlap, roughly where they were, and a row of every
//! workspace along the top. The pointer on a window outlines it (the shell
//! draws its title and close button); a click goes to it. A click on a
//! workspace goes there and the overview stays; a window dragged onto one
//! moves there. Esc, SUPER+W or a click on nothing closes it.
//!
//! The compositor draws the windows (scaled, as they are now: nothing is
//! captured) and handles the pointer; the shell draws the search and the
//! titles over it from the `overview` IPC event. Opening and closing move
//! every window between where it is and where it's spread
//! (`anim::Value`, which the fingers can set too).
//!
//! Where things go is pure and unit-tested here (`spread`, `cells`).

use std::{
    cell::Cell,
    time::{Duration, Instant},
};

use smithay::{
    backend::renderer::{
        element::{solid::SolidColorRenderElement, surface::WaylandSurfaceRenderElement, AsRenderElements, Id, Kind},
        gles::GlesRenderer,
        utils::CommitCounter,
    },
    desktop::{layer_map_for_output, Window},
    output::Output,
    utils::{Logical, Physical, Point, Rectangle, Scale},
    wayland::shell::wlr_layer::Layer as WlrLayer,
};

use crate::{anim, render::OutputElement, state::WhiteRose};

/// How long opening and closing take.
pub const OPEN: Duration = Duration::from_millis(250);
/// Between spread windows.
pub const GAP: f64 = 40.0;
/// Room under each spread window for its title (the shell's).
pub const LABEL: f64 = 36.0;
/// Around the windows' area.
const MARGIN: f64 = 48.0;
/// Below the usable area's top: the search field, then the workspace row.
const ROW_TOP: f64 = 86.0;
/// A workspace in the row at most this wide, this far apart.
const CELL_W: f64 = 112.0;
const CELL_GAP: f64 = 10.0;
/// Pressed and moved this far, it's a drag, not a click.
const DRAG: f64 = 6.0;
/// The dragged window's width.
const DRAGGED_W: f64 = 160.0;
/// The close button's size (the shell draws it on the hovered window's
/// top right corner; this is where it takes clicks).
pub const CLOSE: f64 = 26.0;

// Colours, premultiplied: behind everything, a workspace, an outline, the
// workspace a window is dragged over.
const BACKDROP: [f32; 3] = [0.047, 0.047, 0.051];
const CELL: [f32; 3] = [0.11, 0.11, 0.12];
const OUTLINE: [f32; 3] = [0.96, 0.96, 0.96];
const TARGET: [f32; 3] = [0.91, 0.64, 0.68];

/// A rectangle in logical pixels, relative to an output.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect { x, y, w, h }
    }

    pub fn contains(&self, p: (f64, f64)) -> bool {
        p.0 >= self.x && p.0 < self.x + self.w && p.1 >= self.y && p.1 < self.y + self.h
    }

    fn centre(&self) -> (f64, f64) {
        (self.x + self.w / 2.0, self.y + self.h / 2.0)
    }

    /// `t` of the way from this to `to`.
    pub fn lerp(&self, to: &Rect, t: f64) -> Rect {
        let l = |a: f64, b: f64| a + (b - a) * t;
        Rect::new(l(self.x, to.x), l(self.y, to.y), l(self.w, to.w), l(self.h, to.h))
    }

    fn overlaps(&self, o: &Rect) -> bool {
        self.x < o.x + o.w && o.x < self.x + self.w && self.y < o.y + o.h && o.y < self.y + self.h
    }

    fn physical(&self, scale: f64) -> Rectangle<i32, Physical> {
        let x0 = (self.x * scale).round() as i32;
        let y0 = (self.y * scale).round() as i32;
        let x1 = ((self.x + self.w) * scale).round() as i32;
        let y1 = ((self.y + self.h) * scale).round() as i32;
        Rectangle::new((x0, y0).into(), (x1 - x0, y1 - y0).into())
    }
}

/// Where `windows` (their rectangles now) go in `area`: in rows, each
/// window as big as it is at most, all scaled alike, with `gap` between
/// them and `LABEL` under each row, none overlapping. Rows are filled top
/// to bottom by where the windows are, each row left to right, so they
/// stay roughly where they were. As many rows as make them biggest. Same
/// order as `windows`.
pub fn spread(windows: &[Rect], area: Rect, gap: f64) -> Vec<Rect> {
    let n = windows.len();
    if n == 0 || area.w <= 0.0 || area.h <= 0.0 {
        return Vec::new();
    }
    let mut by_y: Vec<usize> = (0..n).collect();
    by_y.sort_by(|a, b| {
        let (ca, cb) = (windows[*a].centre(), windows[*b].centre());
        ca.1.total_cmp(&cb.1).then(ca.0.total_cmp(&cb.0))
    });
    let rows_for = |count: usize| -> Vec<Vec<usize>> {
        let per = n.div_ceil(count);
        by_y.chunks(per)
            .map(|c| {
                let mut row = c.to_vec();
                row.sort_by(|a, b| windows[*a].centre().0.total_cmp(&windows[*b].centre().0));
                row
            })
            .collect()
    };
    let mut best: Option<(f64, Vec<Vec<usize>>)> = None;
    for count in 1..=n {
        let rows = rows_for(count);
        let r = rows.len() as f64;
        let mut s: f64 = 1.0;
        let mut heights = 0.0;
        for row in &rows {
            let widths: f64 = row.iter().map(|i| windows[*i].w).sum();
            let room = area.w - gap * (row.len() as f64 - 1.0);
            s = s.min(room / widths.max(1.0));
            heights += row.iter().map(|i| windows[*i].h).fold(0.0, f64::max);
        }
        s = s.min((area.h - r * LABEL - gap * (r - 1.0)) / heights.max(1.0));
        if best.as_ref().is_none_or(|(b, _)| s > *b + 1e-9) {
            best = Some((s, rows));
        }
    }
    let (s, rows) = best.expect("at least one row");
    let s = s.max(0.01);
    let row_h: Vec<f64> = rows.iter().map(|row| row.iter().map(|i| windows[*i].h * s).fold(0.0, f64::max)).collect();
    let total = row_h.iter().sum::<f64>() + rows.len() as f64 * LABEL + gap * (rows.len() as f64 - 1.0);
    let mut y = area.y + (area.h - total).max(0.0) / 2.0;
    let mut out = vec![Rect::default(); n];
    for (row, h) in rows.iter().zip(&row_h) {
        let width = row.iter().map(|i| windows[*i].w * s).sum::<f64>() + gap * (row.len() as f64 - 1.0);
        let mut x = area.x + (area.w - width) / 2.0;
        for i in row {
            let (w, wh) = (windows[*i].w * s, windows[*i].h * s);
            out[*i] = Rect::new(x, y + (h - wh) / 2.0, w, wh);
            x += w + gap;
        }
        y += h + LABEL + gap;
    }
    out
}

/// The workspace row on an output `w` × `h` whose usable area starts at
/// `top`: `count` cells shaped like the output, centred.
pub fn cells(w: f64, h: f64, top: f64, count: usize) -> Vec<Rect> {
    let n = count as f64;
    let cw = CELL_W.min((w - 2.0 * MARGIN - (n - 1.0) * CELL_GAP) / n).max(8.0);
    let ch = cw * h / w.max(1.0);
    let x0 = (w - (n * cw + (n - 1.0) * CELL_GAP)) / 2.0;
    (0..count).map(|i| Rect::new(x0 + i as f64 * (cw + CELL_GAP), top + ROW_TOP, cw, ch)).collect()
}

/// Where the windows are spread on that output, under the row.
pub fn window_area(w: f64, h: f64, row: &[Rect]) -> Rect {
    let top = row.first().map_or(0.0, |c| c.y + c.h) + 32.0;
    Rect::new(MARGIN, top, w - 2.0 * MARGIN, h - top - 12.0)
}

/// Whether `p` is on the close button of a window spread at `r`.
pub fn on_close(r: &Rect, p: (f64, f64)) -> bool {
    Rect::new(r.x + r.w - CLOSE / 2.0, r.y - CLOSE / 2.0, CLOSE, CLOSE).contains(p)
}

/// The overview's state, for every output.
#[derive(Debug)]
pub struct Overview {
    /// 0 closed, 1 open.
    pub open: anim::Value,
    /// The window under the pointer.
    pub hovered: Option<Window>,
    /// A window pressed (and where): a click, or a drag once it moves.
    pub pressed: Option<(Window, Point<f64, Logical>)>,
    /// The window being dragged.
    pub dragging: Option<Window>,
    /// Where the pointer is (global).
    pub pointer: Point<f64, Logical>,
    /// The output it opened on (the pointer's): the search is there.
    pub search_on: Option<String>,
    /// For the shapes drawn: stable ids, and bumped when they change.
    ids: Vec<Id>,
    commit: Cell<CommitCounter>,
}

impl Default for Overview {
    fn default() -> Self {
        Overview {
            open: anim::Value::new(0.0),
            hovered: None,
            pressed: None,
            dragging: None,
            pointer: Point::default(),
            search_on: None,
            ids: (0..32).map(|_| Id::new()).collect(),
            commit: Cell::new(CommitCounter::default()),
        }
    }
}

/// One output's overview: each window of the current workspace with where
/// it is and where it's spread, and the workspace row.
#[derive(Debug, Clone)]
pub struct Scene {
    pub output: Output,
    /// The output's place (global).
    pub origin: Point<i32, Logical>,
    pub windows: Vec<(Window, Rect, Rect)>,
    pub cells: Vec<Rect>,
}

impl Scene {
    /// The spread window at `p` (output-local), topmost first.
    fn window_at(&self, p: (f64, f64)) -> Option<(&Window, Rect)> {
        self.windows.iter().rev().find(|(_, _, r)| r.contains(p)).map(|(w, _, r)| (w, *r))
    }

    fn cell_at(&self, p: (f64, f64)) -> Option<usize> {
        self.cells.iter().position(|c| c.contains(p))
    }
}

impl WhiteRose {
    /// Open, opening, closing or held by the fingers: it takes the pointer
    /// and is drawn.
    pub fn overview_active(&self) -> bool {
        let now = Instant::now();
        self.overview.open.target() > 0.0 || self.overview.open.moving(now) || self.overview.open.driven()
    }

    /// How open it is now, 0 to 1.
    pub fn overview_shown(&self) -> f64 {
        self.overview.open.get(Instant::now())
    }

    pub fn open_overview(&mut self) {
        if self.overview_style_off() || self.is_locked() {
            return;
        }
        let now = Instant::now();
        let way = 1.0 - self.overview.open.get(now);
        self.overview.open.animate_to(1.0, OPEN.mul_f64(way), anim::Curve::EaseOut, now);
        let at = self.seat.get_pointer().map(|p| p.current_location()).unwrap_or_default();
        self.overview.pointer = at;
        self.overview.search_on = self.output_at(at).map(|o| o.name());
        self.overview_hover(at);
        self.overview_changed();
    }

    pub fn close_overview(&mut self) {
        if !self.overview_active() {
            return;
        }
        let now = Instant::now();
        let way = self.overview.open.get(now);
        self.overview.open.animate_to(0.0, OPEN.mul_f64(way), anim::Curve::EaseOut, now);
        self.overview.hovered = None;
        self.overview.pressed = None;
        self.overview.dragging = None;
        self.overview_changed();
    }

    pub fn toggle_overview(&mut self) {
        if self.overview.open.target() > 0.0 {
            self.close_overview();
        } else {
            self.open_overview();
        }
    }

    /// The fingers have it `v` open (0 to 1).
    pub(crate) fn drive_overview(&mut self, v: f64) {
        let was = self.overview_active();
        self.overview.open.drive(v.clamp(0.0, 1.0), Instant::now());
        if !was {
            let at = self.seat.get_pointer().map(|p| p.current_location()).unwrap_or_default();
            self.overview.pointer = at;
            self.overview.search_on = self.output_at(at).map(|o| o.name());
        }
        self.overview_changed();
    }

    /// The fingers lifted: it springs open (or closed) from where they
    /// left it, at their speed.
    pub(crate) fn end_overview_drive(&mut self, open: bool) {
        self.overview.open.release(if open { 1.0 } else { 0.0 }, Instant::now());
        if open {
            let at = self.overview.pointer;
            self.overview_hover(at);
        } else {
            self.overview.hovered = None;
            self.overview.pressed = None;
            self.overview.dragging = None;
        }
        self.overview_changed();
    }

    /// Something about it changed: draw, and tell the shell.
    pub(crate) fn overview_changed(&mut self) {
        let c = self.overview.commit.get();
        let mut next = c;
        next.increment();
        self.overview.commit.set(next);
        self.schedule_redraw();
        self.overview_sync();
    }

    /// `overview.style` is off: the overview doesn't open.
    fn overview_style_off(&self) -> bool {
        self.overview_style == "off"
    }

    /// Takes `overview.style`.
    pub fn overview_setting(&mut self, key: &str, value: &serde_json::Value) {
        match key {
            "overview.style" => {
                self.overview_style = value.as_str().unwrap_or("gnome").to_owned();
                if self.overview_style_off() {
                    self.close_overview();
                }
            }
            "overview.superTap" => self.overview_super_tap = value.as_bool().unwrap_or(false),
            _ => {}
        }
    }

    /// The overview on `output`: where each window of the current workspace
    /// is now and where it's spread, and the workspace row.
    pub fn overview_scene(&self, output: &Output) -> Option<Scene> {
        let geo = self.space.output_geometry(output)?;
        let top = (self.usable_area_on(output).loc.y - geo.loc.y) as f64;
        let (w, h) = (geo.size.w as f64, geo.size.h as f64);
        let cells = cells(w, h, top, self.workspaces.count());
        let area = window_area(w, h, &cells);
        let windows: Vec<(Window, Rect)> = self
            .workspaces
            .active_windows()
            .iter()
            .filter(|win| !self.minimized.contains(win) && Some(*win) != self.overview.dragging.as_ref())
            .filter_map(|win| {
                let g = self.space.element_geometry(win)?;
                // On the output its middle is on.
                let mid = g.loc + Point::from((g.size.w / 2, g.size.h / 2));
                if !geo.contains(mid) || g.size.w <= 0 || g.size.h <= 0 {
                    return None;
                }
                let r = Rect::new(
                    (g.loc.x - geo.loc.x) as f64,
                    (g.loc.y - geo.loc.y) as f64,
                    g.size.w as f64,
                    g.size.h as f64,
                );
                Some((win.clone(), r))
            })
            .collect();
        let now: Vec<Rect> = windows.iter().map(|(_, r)| *r).collect();
        let spread = spread(&now, area, GAP);
        let windows = windows.into_iter().zip(spread).map(|((win, r), s)| (win, r, s)).collect();
        Some(Scene { output: output.clone(), origin: geo.loc, windows, cells })
    }

    fn overview_scene_at(&self, pos: Point<f64, Logical>) -> Option<Scene> {
        let o = self.output_at(pos)?;
        self.overview_scene(&o)
    }

    /// The pointer moved to `pos` with the overview open: what's under it
    /// is outlined, and a pressed window that moves is dragged.
    pub fn overview_pointer(&mut self, pos: Point<f64, Logical>) {
        self.overview.pointer = pos;
        if let Some((w, at)) = self.overview.pressed.clone() {
            if self.overview.dragging.is_none() && (pos - at).to_f64().x.hypot((pos - at).y) >= DRAG {
                self.overview.dragging = Some(w);
                self.overview.hovered = None;
            }
        }
        if self.overview.dragging.is_some() {
            self.overview_changed();
            return;
        }
        self.overview_hover(pos);
    }

    fn overview_hover(&mut self, pos: Point<f64, Logical>) {
        let hovered = self.overview_scene_at(pos).and_then(|s| {
            let p = ((pos.x - s.origin.x as f64), (pos.y - s.origin.y as f64));
            s.window_at(p).map(|(w, _)| w.clone())
        });
        if hovered != self.overview.hovered {
            self.overview.hovered = hovered;
            self.overview_changed();
        }
    }

    /// A button with the overview open. A click on a window goes to it; on
    /// its close button closes it; on a workspace goes there; on nothing
    /// closes the overview. A window dragged onto a workspace moves there.
    pub fn overview_button(&mut self, pressed: bool) {
        let pos = self.overview.pointer;
        let Some(scene) = self.overview_scene_at(pos) else { return };
        let p = (pos.x - scene.origin.x as f64, pos.y - scene.origin.y as f64);
        if pressed {
            // The hovered window's close button reaches past its corner.
            if let Some(w) = self.overview.hovered.clone() {
                if scene.windows.iter().any(|(x, _, r)| *x == w && on_close(r, p)) {
                    self.close_window(&w);
                    return;
                }
            }
            if let Some((w, _)) = scene.window_at(p) {
                self.overview.pressed = Some((w.clone(), pos));
                return;
            }
            if let Some(ws) = scene.cell_at(p) {
                self.switch_workspace(ws);
                self.overview_changed();
                return;
            }
            self.close_overview();
            return;
        }
        let pressed = self.overview.pressed.take();
        if let Some(w) = self.overview.dragging.take() {
            if let Some(ws) = scene.cell_at(p) {
                self.move_window_to(&w, ws);
            }
            self.overview_hover(pos);
            self.overview_changed();
            return;
        }
        if let Some((w, _)) = pressed {
            self.close_overview();
            self.activate_window(&w);
        }
    }

    /// Moves `w` to workspace `ws` (hidden if that isn't the one shown).
    pub fn move_window_to(&mut self, w: &Window, ws: usize) {
        if ws < self.workspaces.count() && self.workspaces.workspace_of(w) != Some(ws) {
            self.leave_group(w);
        }
        if self.workspaces.move_to(w, ws) {
            self.end_snap(w);
            self.hide(w);
            self.retile();
            if self.focused_window().as_ref() == Some(w) {
                let next = self.next_focus();
                self.focus_window(next);
            }
            self.ipc_sync();
        }
    }

    /// Closes `w` as its close button would.
    fn close_window(&mut self, w: &Window) {
        crate::win::WinExt::close(w);
    }

    /// What the shell draws over the overview: each output's spread
    /// windows (ids and where, output-local) once it's open, and which is
    /// under the pointer. None when it's closed (or closing).
    pub fn overview_view(&self) -> Option<crate::ipc::OverviewView> {
        if self.overview.open.target() <= 0.0 && !self.overview.open.driven() {
            return None;
        }
        let outputs = self
            .space
            .outputs()
            .filter_map(|o| self.overview_scene(o))
            .map(|s| crate::ipc::OverviewOutput {
                output: s.output.name(),
                windows: s
                    .windows
                    .iter()
                    .map(|(w, _, r)| crate::ipc::OverviewWindow {
                        id: self.window_id(w),
                        x: r.x.round() as i32,
                        y: r.y.round() as i32,
                        width: r.w.round() as i32,
                        height: r.h.round() as i32,
                    })
                    .collect(),
            })
            .collect();
        Some(crate::ipc::OverviewView {
            outputs,
            hovered: self.overview.hovered.as_ref().map(|w| self.window_id(w)),
            dragging: self.overview.dragging.is_some(),
            search: self.overview.search_on.clone().or_else(|| self.space.outputs().next().map(|o| o.name())),
        })
    }

    /// Tells the shell (only when it changed).
    pub fn overview_sync(&mut self) {
        let e = crate::ipc::Event::Overview { overview: self.overview_view() };
        if let Some(server) = self.ipc.as_mut() {
            server.update(e);
        }
    }
}

/// What `output` shows with the overview open (front to back), or None
/// when it isn't.
pub fn elements(renderer: &mut GlesRenderer, output: &Output, state: &WhiteRose) -> Option<Vec<OutputElement>> {
    if !state.overview_active() {
        return None;
    }
    let scene = state.overview_scene(output)?;
    let scale = output.current_scale().fractional_scale();
    let t = state.overview_shown().clamp(0.0, 1.0);
    let ov = &state.overview;
    let mut shapes = Shapes { ids: ov.ids.iter(), commit: ov.commit.get(), scale };
    let mut out = Vec::new();

    // The bar and the shell's layers (the search) over everything.
    {
        let map = layer_map_for_output(output);
        for layer in [WlrLayer::Overlay, WlrLayer::Top] {
            for l in map.layers_on(layer).rev() {
                let loc = map.layer_geometry(l).map(|g| g.loc).unwrap_or_default();
                out.extend(
                    l.render_elements::<WaylandSurfaceRenderElement<_>>(
                        renderer,
                        loc.to_physical_precise_round(scale),
                        Scale::from(scale),
                        1.0,
                    )
                    .into_iter()
                    .map(OutputElement::Surface),
                );
            }
        }
    }

    // The window being dragged, small, under the pointer.
    let local = |p: Point<f64, Logical>| (p.x - scene.origin.x as f64, p.y - scene.origin.y as f64);
    let pointer = local(ov.pointer);
    let target = ov.dragging.as_ref().and_then(|_| scene.cell_at(pointer));
    if let Some(w) = ov.dragging.as_ref() {
        let g = w.geometry();
        if g.size.w > 0 && state.output_at(ov.pointer).as_ref() == Some(output) {
            let k = DRAGGED_W / g.size.w as f64;
            let r = Rect::new(pointer.0 - DRAGGED_W / 2.0, pointer.1 - 12.0, DRAGGED_W, g.size.h as f64 * k);
            scaled_window(renderer, w, r, scale, 0.9, &mut out);
        }
    }

    // Outlines: the window under the pointer, the current workspace, the
    // workspace a window is dragged over.
    if t >= 1.0 {
        if let Some((_, _, r)) = scene.windows.iter().find(|(w, _, _)| Some(w) == ov.hovered.as_ref()) {
            shapes.ring(&mut out, *r, OUTLINE, 1.0);
        }
    }
    if let Some(c) = scene.cells.get(state.workspaces.active()) {
        shapes.ring(&mut out, *c, OUTLINE, t);
    }
    if let Some(c) = target.and_then(|i| scene.cells.get(i)) {
        shapes.ring(&mut out, *c, TARGET, 1.0);
    }

    // The current workspace's windows, from where they are to spread.
    for (w, now, to) in scene.windows.iter().rev() {
        scaled_window(renderer, w, now.lerp(to, t), scale, state.window_alpha(w), &mut out);
    }

    // The workspace row: each workspace's windows on this output, live,
    // then the workspace itself.
    let (ow, oh) = output_size(state, output);
    for (i, cell) in scene.cells.iter().enumerate() {
        let k = cell.w / ow.max(1.0);
        for w in state.workspaces.windows(i).iter().rev() {
            if state.minimized.contains(w) || Some(w) == ov.dragging.as_ref() {
                continue;
            }
            let at = state
                .space
                .element_location(w)
                .or_else(|| state.hidden_at.iter().find(|(h, _)| h == w).map(|(_, l)| *l));
            let Some(at) = at else { continue };
            let size = w.geometry().size;
            let r =
                Rect::new((at.x - scene.origin.x) as f64, (at.y - scene.origin.y) as f64, size.w as f64, size.h as f64);
            if !r.overlaps(&Rect::new(0.0, 0.0, ow, oh)) {
                continue;
            }
            let mini = Rect::new(cell.x + r.x * k, cell.y + r.y * k, r.w * k, r.h * k);
            scaled_window(renderer, w, mini, scale, t as f32, &mut out);
        }
        shapes.solid(&mut out, *cell, CELL, t);
    }

    // Behind it all, fading in over the desktop.
    shapes.solid(&mut out, Rect::new(0.0, 0.0, ow, oh), BACKDROP, t);
    {
        let map = layer_map_for_output(output);
        for layer in [WlrLayer::Bottom, WlrLayer::Background] {
            for l in map.layers_on(layer).rev() {
                let loc = map.layer_geometry(l).map(|g| g.loc).unwrap_or_default();
                out.extend(
                    l.render_elements::<WaylandSurfaceRenderElement<_>>(
                        renderer,
                        loc.to_physical_precise_round(scale),
                        Scale::from(scale),
                        1.0,
                    )
                    .into_iter()
                    .map(OutputElement::Surface),
                );
            }
        }
    }
    Some(out)
}

/// Flat shapes (the backdrop, workspaces, outlines), each with its own
/// stable id.
struct Shapes<'a> {
    ids: std::slice::Iter<'a, Id>,
    commit: CommitCounter,
    scale: f64,
}

impl Shapes<'_> {
    fn solid(&mut self, out: &mut Vec<OutputElement>, r: Rect, c: [f32; 3], a: f64) {
        let Some(id) = self.ids.next() else { return };
        let a = a as f32;
        out.push(OutputElement::Solid(SolidColorRenderElement::new(
            id.clone(),
            r.physical(self.scale),
            self.commit,
            [c[0] * a, c[1] * a, c[2] * a, a],
            Kind::Unspecified,
        )));
    }

    /// A 2 px outline just outside `r`.
    fn ring(&mut self, out: &mut Vec<OutputElement>, r: Rect, c: [f32; 3], a: f64) {
        let w = 2.0;
        self.solid(out, Rect::new(r.x - w, r.y - w, r.w + 2.0 * w, w), c, a);
        self.solid(out, Rect::new(r.x - w, r.y + r.h, r.w + 2.0 * w, w), c, a);
        self.solid(out, Rect::new(r.x - w, r.y, w, r.h), c, a);
        self.solid(out, Rect::new(r.x + r.w, r.y, w, r.h), c, a);
    }
}

fn output_size(state: &WhiteRose, output: &Output) -> (f64, f64) {
    state.space.output_geometry(output).map_or((0.0, 0.0), |g| (g.size.w as f64, g.size.h as f64))
}

/// `w` drawn (live) so its content fills `r` (output-local, logical).
fn scaled_window(
    renderer: &mut GlesRenderer,
    w: &Window,
    r: Rect,
    scale: f64,
    alpha: f32,
    out: &mut Vec<OutputElement>,
) {
    let g = w.geometry();
    if g.size.w <= 0 {
        return;
    }
    let k = r.w / g.size.w as f64;
    let at = Point::<f64, Logical>::from((r.x - g.loc.x as f64 * k, r.y - g.loc.y as f64 * k));
    out.extend(
        w.render_elements::<WaylandSurfaceRenderElement<_>>(
            renderer,
            at.to_physical_precise_round(scale),
            Scale::from(scale * k),
            alpha,
        )
        .into_iter()
        .map(OutputElement::Surface),
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Rect = Rect { x: 48.0, y: 200.0, w: 1344.0, h: 580.0 };

    fn apart(rs: &[Rect]) -> bool {
        rs.iter().enumerate().all(|(i, a)| rs.iter().skip(i + 1).all(|b| !a.overlaps(b)))
    }

    fn inside(r: &Rect, a: &Rect) -> bool {
        r.x >= a.x - 1e-6 && r.y >= a.y - 1e-6 && r.x + r.w <= a.x + a.w + 1e-6 && r.y + r.h <= a.y + a.h + 1e-6
    }

    #[test]
    fn nothing_to_spread() {
        assert!(spread(&[], AREA, GAP).is_empty());
    }

    #[test]
    fn one_small_window_keeps_its_size_and_is_centred() {
        let s = spread(&[Rect::new(10.0, 40.0, 400.0, 300.0)], AREA, GAP);
        assert_eq!((s[0].w, s[0].h), (400.0, 300.0), "never bigger than it is");
        assert!((s[0].x + 200.0 - (AREA.x + AREA.w / 2.0)).abs() < 1e-6);
    }

    #[test]
    fn a_big_window_shrinks_to_fit_with_room_for_its_title() {
        let s = spread(&[Rect::new(0.0, 26.0, 1440.0, 782.0)], AREA, GAP);
        assert!(inside(&s[0], &AREA));
        assert!(s[0].h <= AREA.h - LABEL + 1e-6);
        assert!((s[0].w / s[0].h - 1440.0 / 782.0).abs() < 1e-6, "same shape");
    }

    #[test]
    fn overlapping_windows_end_up_apart_and_where_they_were() {
        // Three windows piled up: left, right and lower middle.
        let ws = [
            Rect::new(40.0, 60.0, 900.0, 600.0),
            Rect::new(500.0, 80.0, 900.0, 600.0),
            Rect::new(300.0, 300.0, 900.0, 500.0),
        ];
        let s = spread(&ws, AREA, GAP);
        assert!(apart(&s));
        assert!(s.iter().all(|r| inside(r, &AREA)));
        assert!(s[0].x < s[1].x, "left stays left of right");
        let scale = s[0].w / 900.0;
        assert!(s.iter().zip(&ws).all(|(a, b)| (a.w / b.w - scale).abs() < 1e-9), "scaled alike");
    }

    #[test]
    fn many_windows_take_more_rows() {
        let ws: Vec<Rect> = (0..9).map(|i| Rect::new(i as f64 * 10.0, i as f64 * 10.0, 800.0, 600.0)).collect();
        let s = spread(&ws, AREA, GAP);
        assert!(apart(&s));
        assert!(s.iter().all(|r| inside(r, &AREA)));
        let rows = {
            let mut ys: Vec<i64> = s.iter().map(|r| r.y.round() as i64).collect();
            ys.sort();
            ys.dedup();
            ys.len()
        };
        assert!(rows >= 2, "{rows}");
    }

    #[test]
    fn the_row_fits_and_is_shaped_like_the_screen() {
        let c = cells(1440.0, 808.0, 26.0, 9);
        assert_eq!(c.len(), 9);
        assert_eq!(c[0].w, CELL_W);
        assert!((c[0].h - CELL_W * 808.0 / 1440.0).abs() < 1e-9);
        assert!((c[0].x - (1440.0 - c[8].x - c[8].w)).abs() < 1e-9, "centred");
        // A narrow screen: smaller cells, still inside.
        let c = cells(800.0, 1280.0, 26.0, 9);
        assert!(c[8].x + c[8].w <= 800.0 - 40.0);
        let a = window_area(1440.0, 808.0, &cells(1440.0, 808.0, 26.0, 9));
        assert!(a.y > c[0].y);
    }

    #[test]
    fn the_close_button_reaches_past_the_corner() {
        let r = Rect::new(100.0, 100.0, 300.0, 200.0);
        assert!(on_close(&r, (400.0, 100.0)));
        assert!(on_close(&r, (410.0, 90.0)));
        assert!(!on_close(&r, (250.0, 200.0)));
    }

    #[test]
    fn lerp_goes_between() {
        let a = Rect::new(0.0, 0.0, 100.0, 100.0);
        let b = Rect::new(100.0, 50.0, 50.0, 50.0);
        assert_eq!(a.lerp(&b, 0.0), a);
        assert_eq!(a.lerp(&b, 1.0), b);
        assert_eq!(a.lerp(&b, 0.5), Rect::new(50.0, 25.0, 75.0, 75.0));
    }
}
