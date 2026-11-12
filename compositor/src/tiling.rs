//! Tiling in the compositor: keeps a [`Tiling`] per (workspace, output) whose
//! layout isn't floating, and places its windows.
//!
//! [`WhiteRose::retile`] reconciles the visible workspace with the layout
//! settings: on each output, the windows that should be tiled join the tree
//! next to the most recently focused tile, those that shouldn't leave it,
//! and every tile gets its rectangle. It runs after anything that changes
//! what's on screen (a window's first commit, closing, minimising, switching
//! or moving workspaces, maximize ending, the bar or outputs changing, the
//! settings). Cheap when nothing changed: a window is only reconfigured when
//! its rectangle moved.
//!
//! A window's floating rectangle is remembered when it's tiled and given
//! back when it floats again (the workspace turns floating, or it moves to
//! a floating one). Dialogs and fixed-size windows always float, above the
//! tiles. Maximized and fullscreen windows keep their place in the tree.

use serde_json::Value;
use smithay::{
    backend::renderer::utils::CommitCounter,
    desktop::Window,
    input::pointer::{Focus, GrabStartData},
    output::Output,
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as ToplevelState,
    utils::{Logical, Point, Rectangle, Serial},
    wayland::{compositor::with_states, shell::xdg::SurfaceCachedState},
};

use crate::{
    grabs::{TileMoveGrab, TileResizeGrab},
    layout::{self, Axis, Dir, Drop, Layout, Rect, Side, Tiling, TAB_STRIP},
    outputs,
    state::WhiteRose,
    tab_strip::{Strip, StripImages},
};

/// The states that tell a client it's tiled: it drops its shadows and
/// rounded corners on those sides.
pub(crate) const TILED: [ToplevelState; 4] =
    [ToplevelState::TiledLeft, ToplevelState::TiledRight, ToplevelState::TiledTop, ToplevelState::TiledBottom];

/// The tiles of one workspace on one output.
#[derive(Debug)]
pub struct TileSet {
    pub workspace: usize,
    pub output: Output,
    pub tiling: Tiling<Window>,
}

#[derive(Debug, Default)]
pub struct TilingState {
    /// The `windows.*` settings.
    pub settings: layout::Settings,
    pub sets: Vec<TileSet>,
    /// Where each tiled window floated before it was tiled.
    pub floating_at: Vec<(Window, Rectangle<i32, Logical>)>,
    /// Windows the user took out of the tiles: they float anywhere.
    pub floating: Vec<Window>,
    /// The tile being dragged: it follows the pointer, not its tile.
    pub dragging: Option<Window>,
    /// Where the dragged tile will land, drawn under the pointer.
    pub preview: DropPreview,
    /// Resize mode (SUPER+R): the arrow keys resize the focused window.
    pub key_resize: bool,
    /// Tabbed windows sharing a tile.
    pub groups: layout::Groups<Window>,
    /// The visible groups' tab strips, and their drawn pictures.
    pub strips: Vec<Strip>,
    pub strip_images: StripImages,
    /// Each tiled window's tile as last placed: it's drawn and takes the
    /// pointer only there, so an app that won't shrink to its tile (a
    /// minimum size) is cut off instead of covering its neighbours.
    pub placed: Vec<(Window, Rectangle<i32, Logical>)>,
}

/// The drop preview: a translucent rectangle (global coordinates).
#[derive(Debug)]
pub struct DropPreview {
    pub rect: Option<Rectangle<i32, Logical>>,
    pub id: smithay::backend::renderer::element::Id,
    /// Bumped when it moves, so the renderer redraws it.
    pub commit: CommitCounter,
}

impl Default for DropPreview {
    fn default() -> Self {
        DropPreview { rect: None, id: smithay::backend::renderer::element::Id::new(), commit: Default::default() }
    }
}

/// The accent rose (#E8A3AE) at 35%, premultiplied.
pub const PREVIEW_COLOR: [f32; 4] = [0.910 * 0.35, 0.639 * 0.35, 0.682 * 0.35, 0.35];

/// Marks a window whose first commit we've seen: by then the client has
/// said whether it's a dialog and what sizes it accepts.
struct Committed;

fn to_rect(r: Rectangle<i32, Logical>) -> Rect {
    Rect::new(r.loc.x, r.loc.y, r.size.w, r.size.h)
}

fn to_rectangle(r: Rect) -> Rectangle<i32, Logical> {
    Rectangle::new((r.x, r.y).into(), (r.w, r.h).into())
}

impl WhiteRose {
    /// Called on every commit of a window's surface; tiles it on the first.
    pub fn window_committed(&mut self, w: &Window) {
        if w.user_data().get::<Committed>().is_none() {
            w.user_data().insert_if_missing(|| Committed);
            self.retile();
        }
    }

    /// Whether a window can be tiled: not a dialog (it has a parent), not
    /// fixed-size, and not taken out of the tiles. Unknown until its first
    /// commit.
    fn tileable(&self, w: &Window) -> bool {
        let Some(t) = w.toplevel() else { return false };
        if w.user_data().get::<Committed>().is_none() || t.parent().is_some() || self.tiling.floating.contains(w) {
            return false;
        }
        !Self::fixed_size(w)
    }

    /// Whether a window has one size only (its minimum is its maximum).
    pub(crate) fn fixed_size(w: &Window) -> bool {
        let Some(t) = w.toplevel() else { return false };
        let (min, max) = with_states(t.wl_surface(), |s| {
            let mut cached = s.cached_state.get::<SurfaceCachedState>();
            let c = cached.current();
            (c.min_size, c.max_size)
        });
        min == max && min.w > 0 && min.h > 0
    }

    /// The layout of workspace `ws` (0-based) on `output`.
    pub fn layout_of(&self, ws: usize, output: &Output) -> Layout {
        let id = outputs::info(output).id;
        self.tiling.settings.layout_for(ws, Some(id.as_str()).filter(|i| !i.is_empty()))
    }

    /// The tiles `w` is in, if it's tiled.
    pub fn tile_set_of(&self, w: &Window) -> Option<&TileSet> {
        self.tiling.sets.iter().find(|s| s.tiling.contains(w))
    }

    pub fn is_tiled(&self, w: &Window) -> bool {
        self.tile_set_of(w).is_some()
    }

    pub(crate) fn maximized_or_fullscreen(w: &Window) -> bool {
        Self::has_state(w, ToplevelState::Maximized) || Self::has_state(w, ToplevelState::Fullscreen)
    }

    /// Brings the visible workspace's tiles in line with the settings and
    /// the windows on screen, and places them.
    pub fn retile(&mut self) {
        let ws = self.workspaces.active();
        let outputs: Vec<Output> = self.space.outputs().cloned().collect();
        // An output that's gone takes its trees with it; its windows join
        // the trees of the outputs they're on now.
        self.tiling.sets.retain(|s| outputs.contains(&s.output));

        let mut tiled: Vec<Window> = Vec::new();
        let mut strips: Vec<Strip> = Vec::new();
        let mut placed: Vec<(Window, Rectangle<i32, Logical>)> = Vec::new();
        for o in &outputs {
            let layout = self.layout_of(ws, o);
            let set = self.tiling.sets.iter().position(|s| s.workspace == ws && s.output == *o);
            if layout == Layout::Floating {
                if let Some(i) = set {
                    self.tiling.sets.remove(i);
                }
                continue;
            }
            // The windows that belong here: on screen, tileable, and on this
            // output (a tiled window stays on its tree's output).
            let members: Vec<Window> = self
                .workspaces
                .windows(ws)
                .iter()
                .filter(|w| self.space.element_location(w).is_some() && self.tileable(w))
                .filter(|w| match self.tile_set_of(w) {
                    Some(s) => s.workspace == ws && s.output == *o,
                    None => self.output_of(w).as_ref() == Some(o),
                })
                .cloned()
                .collect();
            let settings = &self.tiling.settings;
            let i = match set {
                Some(i) if self.tiling.sets[i].tiling.layout() == layout => i,
                Some(i) => {
                    let set = &mut self.tiling.sets[i];
                    set.tiling = Tiling::new(layout, settings, set.tiling.windows()).expect("layout isn't floating");
                    i
                }
                None => {
                    let tiling = Tiling::new(layout, settings, []).expect("layout isn't floating");
                    self.tiling.sets.push(TileSet { workspace: ws, output: o.clone(), tiling });
                    self.tiling.sets.len() - 1
                }
            };
            for w in self.tiling.sets[i].tiling.windows() {
                if !members.contains(&w) {
                    self.tiling.sets[i].tiling.remove(&w);
                }
            }
            let area = to_rect(self.usable_area_on(o));
            for w in &members {
                if self.tiling.sets[i].tiling.contains(w) {
                    continue;
                }
                self.remember_floating_rect(w);
                let tiling = &self.tiling.sets[i].tiling;
                let next_to = self.workspaces.mru().iter().find(|m| *m != w && tiling.contains(m)).cloned();
                self.tiling.sets[i].tiling.insert(w.clone(), next_to.as_ref());
            }
            for (w, r) in self.tiling.sets[i].tiling.arrange(area, &self.tiling.settings.gaps) {
                let mut r = to_rectangle(r);
                // A group's shown window sits under its tab strip.
                if let Some(tabs) = self.tiling.groups.tabs_of(&w) {
                    let h = TAB_STRIP.min(r.size.h);
                    let shown = tabs.iter().position(|t| *t == w).unwrap_or(0);
                    let rect = Rectangle::new(r.loc, (r.size.w, h).into());
                    strips.push(Strip { output: o.clone(), rect, tabs: tabs.to_vec(), shown });
                    r = Rectangle::new((r.loc.x, r.loc.y + h).into(), (r.size.w, r.size.h - h).into());
                }
                if !Self::maximized_or_fullscreen(&w) {
                    self.place(&w, r);
                    placed.push((w.clone(), r));
                }
                tiled.push(w);
            }
        }
        self.tiling.strips = strips;
        self.tiling.placed = placed;

        // A group whose tile is gone (the workspace floats now) ends: its
        // hidden tabs come back, and they all float where they were.
        let loose: Vec<Window> = self
            .tiling
            .groups
            .iter()
            .map(|(_, shown)| shown.clone())
            .filter(|s| !tiled.contains(s) && self.space.element_location(s).is_some())
            .collect();
        for shown in loose {
            for tab in self.tiling.groups.dissolve(&shown) {
                if self.workspaces.workspace_of(&tab) == Some(ws) && !self.minimized.contains(&tab) {
                    self.show(&tab);
                }
            }
        }

        // Windows on screen that were tiled and no longer are float again
        // where they were.
        let floating: Vec<_> = self
            .tiling
            .floating_at
            .iter()
            .filter(|(w, _)| !tiled.contains(w) && self.space.element_location(w).is_some())
            .cloned()
            .collect();
        for (w, rect) in floating {
            self.tiling.floating_at.retain(|(f, _)| *f != w);
            self.float_again(&w, rect);
        }
        self.refit_snapped();
        self.restack();
        self.schedule_redraw();
    }

    /// Notes where a window floats before it's tiled (once: a window moving
    /// between trees keeps its first floating rectangle). A maximized or
    /// fullscreen window floats where it was before that.
    fn remember_floating_rect(&mut self, w: &Window) {
        if self.tiling.floating_at.iter().any(|(f, _)| f == w) {
            return;
        }
        // A snapped window's snap ends; it floats at its own size again.
        if let Some(s) = self.end_snap(w) {
            self.tiling.floating_at.push((w.clone(), s.before));
            return;
        }
        let rect =
            self.restore_geometry.iter().find(|(r, _)| r == w).map(|(_, r)| *r).unwrap_or_else(|| {
                Rectangle::new(self.space.element_location(w).unwrap_or_default(), w.geometry().size)
            });
        self.tiling.floating_at.push((w.clone(), rect));
    }

    /// Gives a window its tile (a dragged one gets it when it's dropped).
    fn place(&mut self, w: &Window, r: Rectangle<i32, Logical>) {
        let Some(t) = w.toplevel() else { return };
        if self.tiling.dragging.as_ref() == Some(w) {
            return;
        }
        t.with_pending_state(|s| {
            s.size = Some(r.size);
            for state in TILED {
                s.states.set(state);
            }
        });
        t.send_pending_configure();
        if self.space.element_location(w) != Some(r.loc) {
            self.space.map_element(w.clone(), r.loc, false);
        }
    }

    /// Puts a window back where it floated, at its old size.
    fn float_again(&mut self, w: &Window, rect: Rectangle<i32, Logical>) {
        let Some(t) = w.toplevel() else { return };
        // (Read before with_pending_state below: it holds the same lock.)
        let filling = Self::maximized_or_fullscreen(w);
        if filling {
            // It stays maximized; restoring it later goes back here.
            if let Some((_, r)) = self.restore_geometry.iter_mut().find(|(r, _)| r == w) {
                *r = rect;
            }
        }
        t.with_pending_state(|s| {
            for state in TILED {
                s.states.unset(state);
            }
            if !filling {
                // A window tiled before it ever drew lets the client choose.
                s.size = Some(rect.size).filter(|sz| sz.w > 0 && sz.h > 0);
            }
        });
        t.send_pending_configure();
        if !filling {
            self.space.map_element(w.clone(), rect.loc, false);
        }
    }

    /// Keeps floating windows (dialogs, maximized windows) above the tiles,
    /// otherwise leaving the stacking order alone.
    pub fn restack(&mut self) {
        let order: Vec<Window> = self.space.elements().cloned().collect();
        let below = |w: &Window| self.is_tiled(w) && !Self::maximized_or_fullscreen(w);
        let wanted: Vec<Window> =
            order.iter().filter(|w| below(w)).chain(order.iter().filter(|w| !below(w))).cloned().collect();
        if wanted != order {
            for w in &wanted {
                self.space.raise_element(w, false);
            }
        }
    }

    /// Drops a closed window from every tree.
    pub(crate) fn forget_tiled(&mut self, w: &Window) {
        self.leave_group(w);
        for s in &mut self.tiling.sets {
            s.tiling.remove(w);
        }
        self.tiling.floating_at.retain(|(f, _)| f != w);
        self.tiling.floating.retain(|f| f != w);
        if self.tiling.dragging.as_ref() == Some(w) {
            self.tiling.dragging = None;
        }
    }

    /// Takes a `windows.*` setting; true if it changed anything.
    pub fn layout_setting(&mut self, key: &str, value: &Value) -> bool {
        let defaults = layout::Settings::default();
        let s = &mut self.tiling.settings;
        let before = s.clone();
        let layout = |v: &Value| v.as_str().and_then(Layout::parse);
        let px = |v: &Value, default: i32| v.as_i64().map(|p| p.clamp(0, 64) as i32).unwrap_or(default);
        match key {
            "windows.layout" => s.global = layout(value).unwrap_or(defaults.global),
            "windows.layout.workspaces" => {
                s.workspaces = value
                    .as_object()
                    .into_iter()
                    .flatten()
                    .filter_map(|(ws, l)| Some((ws.parse::<usize>().ok()?.checked_sub(1)?, layout(l)?)))
                    .collect()
            }
            "windows.layout.monitors" => {
                s.monitors = value
                    .as_object()
                    .into_iter()
                    .flatten()
                    .filter_map(|(id, l)| Some((id.clone(), layout(l)?)))
                    .collect()
            }
            "windows.gaps.inner" => s.gaps.inner = px(value, defaults.gaps.inner),
            "windows.gaps.outer" => s.gaps.outer = px(value, defaults.gaps.outer),
            "windows.snap" => s.snap = value.as_bool().unwrap_or(defaults.snap),
            "windows.snap.zones" => {
                s.snap_zones = value
                    .as_object()
                    .into_iter()
                    .flatten()
                    .filter_map(|(id, grid)| {
                        let raw: Vec<[f64; 4]> = serde_json::from_value(grid.clone()).ok()?;
                        Some((id.clone(), layout::snap::validate_grid(&raw)?))
                    })
                    .collect()
            }
            "windows.gaps.smart" => s.gaps.smart = value.as_bool().unwrap_or(defaults.gaps.smart),
            "windows.master.ratio" => {
                s.master_ratio =
                    value.as_f64().unwrap_or(defaults.master_ratio).clamp(layout::MIN_RATIO, layout::MAX_RATIO)
            }
            "windows.master.side" => s.master_side = value.as_str().and_then(Side::parse).unwrap_or_default(),
            "windows.dwindle.split" => {
                s.dwindle_split = match value.as_str() {
                    Some("horizontal") => Some(Axis::Horizontal),
                    Some("vertical") => Some(Axis::Vertical),
                    _ => None,
                }
            }
            _ => return false,
        }
        *s != before
    }

    /// Applies changed `windows.*` settings to every tree, re-tiles, and
    /// tells the bars.
    pub fn apply_layout_settings(&mut self) {
        let settings = self.tiling.settings.clone();
        for s in &mut self.tiling.sets {
            s.tiling.apply_settings(&settings);
        }
        self.retile();
        self.ipc_sync();
    }
}

/// Tiled windows are cut to their tiles: an app that won't shrink to its
/// tile (a minimum size) is drawn, hit-tested and given frame callbacks only
/// within it, so it never covers its neighbours. Its popups stay whole.
impl WhiteRose {
    /// The part of the screen a tiled window may show on: its tile. Its
    /// popups aren't clipped. None for floating windows, maximized and
    /// fullscreen ones, and a tile being dragged (it follows the pointer).
    pub fn clip_of(&self, w: &Window) -> Option<Rectangle<i32, Logical>> {
        if self.tiling.dragging.as_ref() == Some(w) || Self::maximized_or_fullscreen(w) {
            return None;
        }
        self.tiling.placed.iter().find(|(p, _)| p == w).map(|(_, tile)| *tile)
    }

    /// Whether `output` shows any of `w` (always, for an unclipped window).
    pub fn shows_on(&self, w: &Window, output: &Output) -> bool {
        match self.clip_of(w) {
            None => true,
            Some(clip) => {
                self.space.output_geometry(output).is_some_and(|o| o.overlaps(clip))
                    && self.space.element_bbox(w).is_some_and(|b| b.overlaps(clip))
            }
        }
    }
}

/// Moving and resizing tiles with the mouse (grabs/tile_grab.rs).
impl WhiteRose {
    /// The tiles of set `i`, where they are now.
    fn tiles_of(&self, i: usize) -> Vec<(Window, Rect)> {
        let s = &self.tiling.sets[i];
        s.tiling.arrange(to_rect(self.usable_area_on(&s.output)), &self.tiling.settings.gaps)
    }

    /// The visible workspace's tiled area under `pos`, as a set index.
    fn tiled_area_at(&self, pos: Point<f64, Logical>) -> Option<usize> {
        let ws = self.workspaces.active();
        self.tiling.sets.iter().position(|s| s.workspace == ws && self.usable_area_on(&s.output).to_f64().contains(pos))
    }

    /// Where dropping `w` at `pos` puts it: on which tile (of which set),
    /// that tile's rectangle, and how.
    fn drop_target(&self, w: &Window, pos: Point<f64, Logical>) -> Option<(usize, Window, Rect, Drop)> {
        let i = self.tiled_area_at(pos)?;
        let p = pos.to_i32_round::<i32>();
        let (target, r) = self.tiles_of(i).into_iter().find(|(t, r)| t != w && to_rectangle(*r).contains(p))?;
        let drop = match layout::drop_zone(r, p.x, p.y) {
            // A tab leaves its group's place to the group: it splits instead.
            Drop::Swap if self.tiling.groups.tabs_of(w).is_some() => Drop::Side(layout::nearest_side(r, p.x, p.y)),
            drop => drop,
        };
        Some((i, target, r, drop))
    }

    /// Starts dragging a tile with the pointer.
    pub fn start_tile_move(&mut self, w: &Window, start_data: GrabStartData<WhiteRose>, serial: Serial) {
        let Some(loc) = self.space.element_location(w) else { return };
        let Some(pointer) = self.seat.get_pointer() else { return };
        self.tiling.dragging = Some(w.clone());
        let grab = TileMoveGrab { start_data, window: w.clone(), initial_window_location: loc };
        pointer.set_grab(self, grab, serial, Focus::Clear);
    }

    /// Shows where the dragged tile `w` lands if dropped at `pos`: half of
    /// the tile it splits, the tile it swaps with, or the whole of an empty
    /// tiled area. Nothing where it would float.
    pub fn show_drop_preview(&mut self, w: &Window, pos: Point<f64, Logical>) {
        let rect = match self.drop_target(w, pos) {
            Some((_, _, r, drop)) => Some(drop.preview(r)),
            None => self.tiled_area_at(pos).filter(|i| self.tiles_of(*i).is_empty()).map(|i| {
                let area = to_rect(self.usable_area_on(&self.tiling.sets[i].output));
                self.tiling.settings.gaps.apply(area, 1).0
            }),
        };
        self.set_preview(rect.map(to_rectangle));
    }

    pub(crate) fn set_preview(&mut self, rect: Option<Rectangle<i32, Logical>>) {
        if self.tiling.preview.rect != rect {
            self.tiling.preview.rect = rect;
            self.tiling.preview.commit.increment();
            self.schedule_redraw();
        }
    }

    /// The drag ended (dropped, or the grab was taken away).
    pub fn end_tile_drag(&mut self) {
        self.tiling.dragging = None;
        self.set_preview(None);
    }

    /// Drops the dragged tile `w` at `pos`: it joins the group of the tile
    /// whose top band it's on, swaps with or splits the tile there, joins
    /// an empty tiled area, or, outside every tiled area, floats where it
    /// is. Anywhere else it goes back to its place. A tab that goes
    /// somewhere else leaves its group.
    pub fn drop_tile(&mut self, w: &Window, pos: Point<f64, Logical>) {
        self.end_tile_drag();
        let Some(from) = self.tiling.sets.iter().position(|s| s.tiling.contains(w)) else { return };
        match self.drop_target(w, pos) {
            Some((i, target, _, Drop::Group)) => self.join_group(w, &target, i),
            Some((i, target, _, Drop::Swap)) if i == from => {
                self.tiling.sets[i].tiling.swap(w, &target);
            }
            Some((i, target, _, Drop::Swap)) => {
                self.tiling.sets[from].tiling.replace(w, target.clone());
                self.tiling.sets[i].tiling.replace(&target, w.clone());
            }
            Some((i, target, _, Drop::Side(side))) => {
                self.leave_group(w);
                self.tiling.sets[from].tiling.remove(w);
                self.tiling.sets[i].tiling.insert_at(w.clone(), &target, side);
            }
            None => match self.tiled_area_at(pos) {
                Some(i) if i != from && self.tiles_of(i).is_empty() => {
                    self.leave_group(w);
                    self.tiling.sets[from].tiling.remove(w);
                    self.tiling.sets[i].tiling.insert(w.clone(), None);
                }
                Some(_) => {} // between tiles, or on its own place: back it goes
                None => self.float_window(w),
            },
        }
        self.retile();
    }

    /// Makes `w` a tab of `target`'s group in set `i`, shown in its place.
    fn join_group(&mut self, w: &Window, target: &Window, i: usize) {
        self.leave_group(w);
        for s in &mut self.tiling.sets {
            s.tiling.remove(w);
        }
        if let Some(old) = self.tiling.groups.join(w.clone(), target) {
            self.tiling.sets[i].tiling.replace(&old, w.clone());
            self.hide(&old);
        }
    }

    /// Takes `w` out of its group. If it was the shown tab, the next one
    /// takes its place in the tiles.
    pub(crate) fn leave_group(&mut self, w: &Window) {
        let Some(next) = self.tiling.groups.leave(w) else { return };
        for s in &mut self.tiling.sets {
            s.tiling.replace(w, next.clone());
        }
        if self.workspaces.workspace_of(&next) == Some(self.workspaces.active()) && !self.minimized.contains(&next) {
            self.show(&next);
        }
    }

    /// Shows tab `w` of its group in place of the shown one.
    pub fn show_tab(&mut self, w: &Window) {
        let Some(old) = self.tiling.groups.show(w) else { return };
        for s in &mut self.tiling.sets {
            s.tiling.replace(&old, w.clone());
        }
        self.hide(&old);
        if self.workspaces.workspace_of(w) == Some(self.workspaces.active()) {
            self.show(w);
        }
        self.retile();
    }

    /// The tab under `pos`, on a visible tab strip.
    pub fn tab_at(&self, pos: Point<f64, Logical>) -> Option<Window> {
        self.tiling.strips.iter().find_map(|s| s.tab_at(pos)).cloned()
    }

    /// Takes a window out of the tiles: it floats where it is, at its tile's
    /// size, until it's tiled again.
    pub fn float_window(&mut self, w: &Window) {
        self.leave_group(w);
        for s in &mut self.tiling.sets {
            s.tiling.remove(w);
        }
        self.tiling.floating_at.retain(|(f, _)| f != w);
        if !self.tiling.floating.contains(w) {
            self.tiling.floating.push(w.clone());
        }
        if let Some(t) = w.toplevel() {
            t.with_pending_state(|s| {
                for state in TILED {
                    s.states.unset(state);
                }
            });
            t.send_pending_configure();
        }
        self.retile();
    }

    /// Starts moving the edges `edges` of tile `w` with the pointer.
    pub fn start_tile_resize(
        &mut self,
        w: &Window,
        edges: Vec<Dir>,
        start_data: GrabStartData<WhiteRose>,
        serial: Serial,
    ) {
        let Some(snapshot) = self.tile_set_of(w).map(|s| s.tiling.clone()) else { return };
        let Some(pointer) = self.seat.get_pointer() else { return };
        let grab = TileResizeGrab { start_data, window: w.clone(), edges, snapshot };
        pointer.set_grab(self, grab, serial, Focus::Clear);
    }

    /// Resizes tile `w` from how its tiles were (`snapshot`): each edge
    /// moves outwards by its pixels. Ignored if the tiles changed meanwhile.
    pub fn resize_tile(&mut self, snapshot: &Tiling<Window>, w: &Window, moves: &[(Dir, i32)]) {
        let Some(i) = self.tiling.sets.iter().position(|s| s.tiling.contains(w)) else { return };
        if self.tiling.sets[i].tiling.windows() != snapshot.windows() {
            return;
        }
        let area = to_rect(self.usable_area_on(&self.tiling.sets[i].output));
        let gaps = self.tiling.settings.gaps;
        let mut tiling = snapshot.clone();
        for (edge, px) in moves {
            tiling.resize_edge(w, *edge, *px, area, &gaps);
        }
        self.tiling.sets[i].tiling = tiling;
        self.retile();
    }

    /// The tile edges at `pos`, when it's in the gap between tiles: the
    /// tile, and which of its edges (only ones that can move).
    pub fn gap_at(&self, pos: Point<f64, Logical>) -> Option<(Window, Vec<Dir>)> {
        let i = self.tiled_area_at(pos)?;
        let p = pos.to_i32_round::<i32>();
        let tiles = self.tiles_of(i);
        if tiles.iter().any(|(_, r)| to_rectangle(*r).contains(p)) {
            return None;
        }
        // Within half the gap of a tile (and a few pixels more, so thin gaps
        // can be caught too).
        let reach = self.tiling.settings.gaps.inner / 2 + 4;
        let area = to_rect(self.usable_area_on(&self.tiling.sets[i].output));
        let gaps = self.tiling.settings.gaps;
        tiles.iter().find_map(|(w, r)| {
            let grown = Rect::new(r.x - reach, r.y - reach, r.w + 2 * reach, r.h + 2 * reach);
            if !to_rectangle(grown).contains(p) {
                return None;
            }
            let mut edges = Vec::new();
            for (beyond, edge) in [
                (p.x < r.x, Dir::Left),
                (p.x >= r.right(), Dir::Right),
                (p.y < r.y, Dir::Up),
                (p.y >= r.bottom(), Dir::Down),
            ] {
                // Only edges on a split: the area's border can't move.
                if beyond && self.tiling.sets[i].tiling.clone().resize_edge(w, edge, 0, area, &gaps) {
                    edges.push(edge);
                }
            }
            (!edges.is_empty()).then(|| (w.clone(), edges))
        })
    }
}

/// How far one key press moves or resizes a window, in pixels.
const KEY_STEP: i32 = 40;

/// The tiling keys (keybind.rs actions).
impl WhiteRose {
    /// The focused window, its tiles (set index) and their area, if tiled.
    fn focused_tile(&self) -> Option<(Window, usize, Rect)> {
        let w = self.focused_window()?;
        let i = self.tiling.sets.iter().position(|s| s.tiling.contains(&w))?;
        let area = to_rect(self.usable_area_on(&self.tiling.sets[i].output));
        Some((w, i, area))
    }

    /// Changes the focused tile's tiles with `f` (given the window, its
    /// area and the gaps), and re-tiles if it did something.
    fn change_tiles(&mut self, f: impl FnOnce(&mut Tiling<Window>, &Window, Rect, &layout::Gaps) -> bool) {
        let Some((w, i, area)) = self.focused_tile() else { return };
        let gaps = self.tiling.settings.gaps;
        if f(&mut self.tiling.sets[i].tiling, &w, area, &gaps) {
            self.retile();
        }
    }

    /// SUPER+T and the bar's layout button: the next layout on the output
    /// named (a bar's screen), else where you're working (the focused
    /// window's output, else the pointer's). It changes the workspace's
    /// layout, or the monitor's if that monitor has its own, and saves it.
    pub fn next_window_layout(&mut self, output: Option<&str>) {
        let ws = self.workspaces.active();
        let named = output.and_then(|n| self.space.outputs().find(|o| o.name() == n).cloned());
        let output =
            named.or_else(|| self.focused_window().and_then(|w| self.output_of(&w))).or_else(|| self.pointer_output());
        let Some(o) = output else { return };
        let next = self.layout_of(ws, &o).next();
        let id = outputs::info(&o).id;
        let s = &mut self.tiling.settings;
        let (key, value) = if !id.is_empty() && s.monitors.contains_key(&id) {
            s.monitors.insert(id, next);
            let map: serde_json::Map<String, Value> =
                s.monitors.iter().map(|(m, l)| (m.clone(), Value::from(l.as_str()))).collect();
            ("windows.layout.monitors", map)
        } else {
            s.workspaces.insert(ws, next);
            let map: serde_json::Map<String, Value> =
                s.workspaces.iter().map(|(w, l)| ((w + 1).to_string(), Value::from(l.as_str()))).collect();
            ("windows.layout.workspaces", map)
        };
        self.daemon_request(serde_json::json!({"op": "set", "key": key, "value": value}));
        self.apply_layout_settings();
    }

    /// SUPER+SHIFT+SPACE: a tile floats where it is; a window floated that
    /// way goes back into the tiles.
    pub fn toggle_floating(&mut self) {
        let Some(w) = self.focused_window() else { return };
        if self.is_tiled(&w) {
            self.float_window(&w);
        } else if self.tiling.floating.contains(&w) {
            self.tiling.floating.retain(|f| *f != w);
            self.retile();
        }
    }

    /// SUPER+arrow: focuses the window beside the focused one (tiled or
    /// not, on any output).
    pub fn focus_direction(&mut self, dir: Dir) {
        let Some(w) = self.focused_window() else { return };
        // Tiled windows count where their tiles are, even if the app draws
        // bigger.
        let windows: Vec<(Window, Rect)> = self
            .space
            .elements()
            .filter_map(|e| {
                let mut r = self.space.element_geometry(e)?;
                if let Some(clip) = self.clip_of(e).filter(|_| *e != w) {
                    r = r.intersection(clip)?;
                }
                Some((e.clone(), to_rect(r)))
            })
            .collect();
        if let Some(n) = layout::neighbour(&windows, &w, dir) {
            self.focus_window(Some(n));
        }
    }

    /// SUPER+SHIFT+arrow: a tile swaps with the one beside it; a floating
    /// window snaps (snapping.rs), or moves a step where it can't.
    pub fn move_direction(&mut self, dir: Dir) {
        if self.focused_tile().is_some() {
            self.change_tiles(|t, w, area, gaps| t.move_dir(w, dir, area, gaps));
            return;
        }
        if self.snap_key(dir) {
            return;
        }
        let Some(w) = self.focused_window().filter(|w| !Self::maximized_or_fullscreen(w)) else { return };
        let Some(mut at) = self.space.element_location(&w) else { return };
        match dir {
            Dir::Left => at.x -= KEY_STEP,
            Dir::Right => at.x += KEY_STEP,
            Dir::Up => at.y -= KEY_STEP,
            Dir::Down => at.y += KEY_STEP,
        }
        // Never under the bar, where it couldn't be grabbed again.
        if let Some(o) = self.output_of(&w) {
            at.y = crate::grabs::move_grab::keep_below(at.y, self.usable_area_on(&o).loc.y);
        }
        self.space.map_element(w, at, false);
        self.schedule_redraw();
    }

    /// SUPER+CTRL+arrow: right and down grow the focused window a step,
    /// left and up shrink it (a tile moves its split).
    pub fn resize_direction(&mut self, dir: Dir) {
        let delta = if dir.forward() { KEY_STEP } else { -KEY_STEP };
        if self.focused_tile().is_some() {
            self.change_tiles(|t, w, area, gaps| t.grow(w, dir.axis(), delta, area, gaps));
            return;
        }
        let Some(w) = self.focused_window().filter(|w| !Self::maximized_or_fullscreen(w)) else { return };
        let Some(t) = w.toplevel() else { return };
        let mut size = w.geometry().size;
        match dir.axis() {
            Axis::Horizontal => size.w = (size.w + delta).max(KEY_STEP),
            Axis::Vertical => size.h = (size.h + delta).max(KEY_STEP),
        }
        t.with_pending_state(|s| s.size = Some(size));
        t.send_pending_configure();
    }

    /// SUPER+J (dwindle): flips the split the focused tile is in.
    pub fn toggle_split(&mut self) {
        self.change_tiles(|t, w, area, gaps| t.toggle_split(w, area, gaps));
    }

    /// SUPER+M (master): makes the focused tile the master.
    pub fn make_master(&mut self) {
        self.change_tiles(|t, w, _, _| t.make_master(w));
    }

    /// SUPER+G: a tab leaves its group (and gets its own tile beside it);
    /// a tile joins the group of the last tile used before it.
    pub fn toggle_group(&mut self) {
        let Some((w, i, _)) = self.focused_tile() else { return };
        if self.tiling.groups.tabs_of(&w).is_some() {
            self.leave_group(&w);
        } else {
            let tiles = &self.tiling.sets[i].tiling;
            let Some(target) = self.workspaces.mru().iter().find(|m| **m != w && tiles.contains(m)).cloned() else {
                return;
            };
            self.join_group(&w, &target, i);
        }
        self.retile();
    }

    /// SUPER+TAB / SUPER+SHIFT+TAB: the next or previous tab of the
    /// focused group.
    pub fn next_tab(&mut self, forward: bool) {
        let Some(w) = self.focused_window() else { return };
        if let Some(tab) = self.tiling.groups.neighbour_tab(&w, forward) {
            self.focus_window(Some(tab));
        }
    }
}
