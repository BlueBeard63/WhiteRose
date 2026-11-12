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
    desktop::Window,
    output::Output,
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as ToplevelState,
    utils::{Logical, Rectangle},
    wayland::{compositor::with_states, shell::xdg::SurfaceCachedState},
};

use crate::{
    layout::{self, Axis, Layout, Rect, Side, Tiling},
    outputs,
    state::WhiteRose,
};

/// The states that tell a client it's tiled: it drops its shadows and
/// rounded corners on those sides.
const TILED: [ToplevelState; 4] =
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
}

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

    /// Whether a window can be tiled: not a dialog (it has a parent) and
    /// not fixed-size. Unknown until its first commit.
    fn tileable(&self, w: &Window) -> bool {
        let Some(t) = w.toplevel() else { return false };
        if w.user_data().get::<Committed>().is_none() || t.parent().is_some() {
            return false;
        }
        let (min, max) = with_states(t.wl_surface(), |s| {
            let mut cached = s.cached_state.get::<SurfaceCachedState>();
            let c = cached.current();
            (c.min_size, c.max_size)
        });
        !(min == max && min.w > 0 && min.h > 0)
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

    fn maximized_or_fullscreen(w: &Window) -> bool {
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
            for w in &members {
                if self.tiling.sets[i].tiling.contains(w) {
                    continue;
                }
                self.remember_floating_rect(w);
                let tiling = &self.tiling.sets[i].tiling;
                let next_to = self.workspaces.mru().iter().find(|m| *m != w && tiling.contains(m)).cloned();
                self.tiling.sets[i].tiling.insert(w.clone(), next_to.as_ref());
            }
            let area = to_rect(self.usable_area_on(o));
            for (w, r) in self.tiling.sets[i].tiling.arrange(area, &self.tiling.settings.gaps) {
                if !Self::maximized_or_fullscreen(&w) {
                    self.place(&w, to_rectangle(r));
                }
                tiled.push(w);
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
        let rect =
            self.restore_geometry.iter().find(|(r, _)| r == w).map(|(_, r)| *r).unwrap_or_else(|| {
                Rectangle::new(self.space.element_location(w).unwrap_or_default(), w.geometry().size)
            });
        self.tiling.floating_at.push((w.clone(), rect));
    }

    /// Gives a window its tile.
    fn place(&mut self, w: &Window, r: Rectangle<i32, Logical>) {
        let Some(t) = w.toplevel() else { return };
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
        for s in &mut self.tiling.sets {
            s.tiling.remove(w);
        }
        self.tiling.floating_at.retain(|(f, _)| f != w);
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

    /// Applies changed `windows.*` settings to every tree, and re-tiles.
    pub fn apply_layout_settings(&mut self) {
        let settings = self.tiling.settings.clone();
        for s in &mut self.tiling.sets {
            s.tiling.apply_settings(&settings);
        }
        self.retile();
    }
}
