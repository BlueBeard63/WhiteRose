//! Tiling layouts: which layout a workspace uses, and the window
//! arrangements that compute each tile's rectangle.
//!
//! Like `workspace.rs`, everything here is generic over the window type and
//! knows nothing about Smithay, so it's unit tested without a compositor.
//! The compositor keeps one [`Tiling`] per (workspace, output) whose layout
//! isn't floating, and places windows from [`Tiling::arrange`].

mod dwindle;
mod group;
mod master;
pub mod snap;

use std::collections::HashMap;

pub use dwindle::Dwindle;
pub use group::{Groups, TAB_STRIP};
pub use master::{Master, Side};

/// How a workspace places its windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Layout {
    /// Windows go where they're put (the cascade).
    #[default]
    Floating,
    /// Each new window splits the focused tile in two.
    Dwindle,
    /// One large master tile and a stack of the rest.
    Master,
}

impl Layout {
    pub const ALL: [Layout; 3] = [Layout::Floating, Layout::Dwindle, Layout::Master];

    /// The name used in settings and IPC.
    pub fn as_str(self) -> &'static str {
        match self {
            Layout::Floating => "floating",
            Layout::Dwindle => "dwindle",
            Layout::Master => "master",
        }
    }

    pub fn parse(s: &str) -> Option<Layout> {
        Layout::ALL.into_iter().find(|l| l.as_str() == s)
    }

    /// The next layout in the cycle (SUPER+T, the bar's button).
    pub fn next(self) -> Layout {
        let i = Layout::ALL.iter().position(|l| *l == self).unwrap_or(0);
        Layout::ALL[(i + 1) % Layout::ALL.len()]
    }
}

/// A rectangle in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

impl Rect {
    pub const fn new(x: i32, y: i32, w: i32, h: i32) -> Rect {
        Rect { x, y, w, h }
    }

    pub fn right(&self) -> i32 {
        self.x + self.w
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.h
    }

    /// The rectangle with `by` pixels taken off every side (never negative).
    pub fn shrink(&self, by: i32) -> Rect {
        Rect::new(self.x + by, self.y + by, (self.w - 2 * by).max(0), (self.h - 2 * by).max(0))
    }

    /// Cuts the rectangle in two along `axis`, `gap` pixels apart, the first
    /// part taking `ratio` of the space left after the gap.
    pub fn split(&self, axis: Axis, ratio: f64, gap: i32) -> (Rect, Rect) {
        let extent = self.extent(axis);
        let gap = gap.min(extent);
        let avail = extent - gap;
        let first = (avail as f64 * ratio).round() as i32;
        let second = avail - first;
        match axis {
            Axis::Horizontal => {
                (Rect::new(self.x, self.y, first, self.h), Rect::new(self.x + first + gap, self.y, second, self.h))
            }
            Axis::Vertical => {
                (Rect::new(self.x, self.y, self.w, first), Rect::new(self.x, self.y + first + gap, self.w, second))
            }
        }
    }

    /// Cuts the rectangle into `n` equal parts along `axis`, `gap` pixels
    /// apart. Leftover pixels go to the first parts, so the parts fill it.
    pub fn divide(&self, axis: Axis, n: usize, gap: i32) -> Vec<Rect> {
        if n == 0 {
            return Vec::new();
        }
        let n32 = n as i32;
        let gap = gap.min(self.extent(axis) / n32.max(2));
        let avail = (self.extent(axis) - gap * (n32 - 1)).max(0);
        let (each, extra) = (avail / n32, avail % n32);
        let mut at = 0;
        (0..n32)
            .map(|i| {
                let size = each + i32::from(i < extra);
                let r = match axis {
                    Axis::Horizontal => Rect::new(self.x + at, self.y, size, self.h),
                    Axis::Vertical => Rect::new(self.x, self.y + at, self.w, size),
                };
                at += size + gap;
                r
            })
            .collect()
    }

    /// Width for a horizontal axis, height for a vertical one.
    pub fn extent(&self, axis: Axis) -> i32 {
        match axis {
            Axis::Horizontal => self.w,
            Axis::Vertical => self.h,
        }
    }

    /// The axis that cuts this rectangle into the squarest halves: side by
    /// side when it's wider than tall, otherwise one above the other.
    pub fn long_axis(&self) -> Axis {
        if self.w >= self.h {
            Axis::Horizontal
        } else {
            Axis::Vertical
        }
    }
}

/// Which way a split runs. Horizontal puts the halves side by side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

impl Axis {
    pub fn other(self) -> Axis {
        match self {
            Axis::Horizontal => Axis::Vertical,
            Axis::Vertical => Axis::Horizontal,
        }
    }
}

/// A direction on screen (keys, drop sides, window edges).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dir {
    Left,
    Right,
    Up,
    Down,
}

impl Dir {
    pub fn axis(self) -> Axis {
        match self {
            Dir::Left | Dir::Right => Axis::Horizontal,
            Dir::Up | Dir::Down => Axis::Vertical,
        }
    }

    /// Right and down: towards the second half of a split.
    pub fn forward(self) -> bool {
        matches!(self, Dir::Right | Dir::Down)
    }

    pub fn opposite(self) -> Dir {
        match self {
            Dir::Left => Dir::Right,
            Dir::Right => Dir::Left,
            Dir::Up => Dir::Down,
            Dir::Down => Dir::Up,
        }
    }
}

/// Space between tiles (`inner`) and around them (`outer`), in pixels.
/// With `smart`, a lone tile gets no gaps at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Gaps {
    pub inner: i32,
    pub outer: i32,
    pub smart: bool,
}

impl Default for Gaps {
    fn default() -> Self {
        Gaps { inner: 8, outer: 8, smart: false }
    }
}

impl Gaps {
    /// The area the tiles share and the gap between them, for `n` tiles.
    pub fn apply(&self, area: Rect, n: usize) -> (Rect, i32) {
        if self.smart && n <= 1 {
            (area, 0)
        } else {
            (area.shrink(self.outer.max(0)), self.inner.max(0))
        }
    }
}

/// Split ratios stay within this range so no tile disappears.
pub const MIN_RATIO: f64 = 0.1;
pub const MAX_RATIO: f64 = 0.9;

fn clamp_ratio(r: f64) -> f64 {
    r.clamp(MIN_RATIO, MAX_RATIO)
}

/// The user's layout settings (the `windows.*` keys).
#[derive(Debug, Clone, PartialEq)]
pub struct Settings {
    /// Used where neither the monitor nor the workspace sets a layout.
    pub global: Layout,
    /// Per workspace (0-based index).
    pub workspaces: HashMap<usize, Layout>,
    /// Per monitor, keyed by its EDID-based id (`monitor_id.rs`).
    pub monitors: HashMap<String, Layout>,
    pub gaps: Gaps,
    /// The master tile's share of the area.
    pub master_ratio: f64,
    pub master_side: Side,
    /// Dwindle's split direction: None picks it from each tile's shape.
    pub dwindle_split: Option<Axis>,
    /// Floating windows snap to edges, corners and zones (`snap.rs`).
    pub snap: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            global: Layout::Floating,
            workspaces: HashMap::new(),
            monitors: HashMap::new(),
            gaps: Gaps::default(),
            master_ratio: 0.55,
            master_side: Side::Left,
            dwindle_split: None,
            snap: true,
        }
    }
}

impl Settings {
    /// The layout of workspace `ws` on the monitor `monitor`: the monitor's
    /// override, else the workspace's, else the global default.
    pub fn layout_for(&self, ws: usize, monitor: Option<&str>) -> Layout {
        monitor.and_then(|m| self.monitors.get(m)).or_else(|| self.workspaces.get(&ws)).copied().unwrap_or(self.global)
    }
}

/// The tiles of one workspace on one output.
#[derive(Debug, Clone, PartialEq)]
pub enum Tiling<W> {
    Dwindle(Dwindle<W>),
    Master(Master<W>),
}

impl<W: Clone + PartialEq> Tiling<W> {
    /// Tiles `windows` (first to last) with `layout`. None for floating.
    pub fn new(layout: Layout, settings: &Settings, windows: impl IntoIterator<Item = W>) -> Option<Tiling<W>> {
        match layout {
            Layout::Floating => None,
            Layout::Dwindle => {
                let mut d = Dwindle::new(settings.dwindle_split);
                for w in windows {
                    let last = d.last().cloned();
                    d.insert(w, last.as_ref());
                }
                Some(Tiling::Dwindle(d))
            }
            Layout::Master => {
                let mut m = Master::new(settings.master_ratio, settings.master_side);
                for w in windows {
                    m.push(w);
                }
                Some(Tiling::Master(m))
            }
        }
    }

    pub fn layout(&self) -> Layout {
        match self {
            Tiling::Dwindle(_) => Layout::Dwindle,
            Tiling::Master(_) => Layout::Master,
        }
    }

    /// Takes on changed settings (split rule, master ratio and side).
    pub fn apply_settings(&mut self, settings: &Settings) {
        match self {
            Tiling::Dwindle(d) => d.split_rule = settings.dwindle_split,
            Tiling::Master(m) => {
                m.ratio = clamp_ratio(settings.master_ratio);
                m.side = settings.master_side;
            }
        }
    }

    /// The tiled windows in reading order (dwindle: tree order; master:
    /// master first), which also seeds a new layout when cycling.
    pub fn windows(&self) -> Vec<W> {
        match self {
            Tiling::Dwindle(d) => d.windows(),
            Tiling::Master(m) => m.windows().to_vec(),
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Tiling::Dwindle(d) => d.len(),
            Tiling::Master(m) => m.windows().len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn contains(&self, w: &W) -> bool {
        match self {
            Tiling::Dwindle(d) => d.contains(w),
            Tiling::Master(m) => m.windows().contains(w),
        }
    }

    /// Adds a window next to `focused` (dwindle splits that tile; master
    /// puts it after it in the stack). Ignored if already tiled.
    pub fn insert(&mut self, w: W, focused: Option<&W>) {
        if self.contains(&w) {
            return;
        }
        match self {
            Tiling::Dwindle(d) => d.insert(w, focused),
            Tiling::Master(m) => m.insert(w, focused),
        }
    }

    /// Adds a window on the `side` of `target` (a mouse drop). Without a
    /// known target it's added like [`Tiling::insert`]. Ignored if already
    /// tiled.
    pub fn insert_at(&mut self, w: W, target: &W, side: Dir) {
        if self.contains(&w) {
            return;
        }
        match self {
            Tiling::Dwindle(d) => d.insert_at(w, target, side),
            Tiling::Master(m) => m.insert_at(w, target, side),
        }
    }

    /// Takes a window out; its neighbours fill the space. False if it
    /// wasn't tiled.
    pub fn remove(&mut self, w: &W) -> bool {
        match self {
            Tiling::Dwindle(d) => d.remove(w),
            Tiling::Master(m) => m.remove(w),
        }
    }

    /// Swaps two tiles. False unless both are tiled here.
    pub fn swap(&mut self, a: &W, b: &W) -> bool {
        if a == b || !self.contains(a) || !self.contains(b) {
            return false;
        }
        match self {
            Tiling::Dwindle(d) => d.swap(a, b),
            Tiling::Master(m) => m.swap(a, b),
        }
        true
    }

    /// Puts `new` in `old`'s place (a tile swapped with one on another
    /// output). False unless `old` is tiled here and `new` isn't.
    pub fn replace(&mut self, old: &W, new: W) -> bool {
        if !self.contains(old) || self.contains(&new) {
            return false;
        }
        match self {
            Tiling::Dwindle(d) => d.replace(old, new),
            Tiling::Master(m) => m.replace(old, new),
        }
        true
    }

    /// Every tile's rectangle within `area`.
    pub fn arrange(&self, area: Rect, gaps: &Gaps) -> Vec<(W, Rect)> {
        match self {
            Tiling::Dwindle(d) => d.arrange(area, gaps),
            Tiling::Master(m) => m.arrange(area, gaps),
        }
    }

    /// The tile next to `w` in direction `dir`, as arranged in `area`.
    pub fn neighbour(&self, w: &W, dir: Dir, area: Rect, gaps: &Gaps) -> Option<W> {
        neighbour(&self.arrange(area, gaps), w, dir)
    }

    /// Swaps `w` with its neighbour in direction `dir`. False if it has none.
    pub fn move_dir(&mut self, w: &W, dir: Dir, area: Rect, gaps: &Gaps) -> bool {
        match self.neighbour(w, dir, area, gaps) {
            Some(n) => self.swap(w, &n),
            None => false,
        }
    }

    /// Moves `w`'s `edge` outwards by `delta` pixels (inwards if negative),
    /// changing the split that edge sits on; the tiles across it follow.
    /// False if that edge is the area's border.
    pub fn resize_edge(&mut self, w: &W, edge: Dir, delta: i32, area: Rect, gaps: &Gaps) -> bool {
        match self {
            Tiling::Dwindle(d) => d.resize_edge(w, edge, delta, area, gaps),
            Tiling::Master(m) => m.resize_edge(w, edge, delta, area, gaps),
        }
    }

    /// Grows `w` by `delta` pixels along `axis` (shrinks if negative): moves
    /// its right or bottom edge, or the left or top one when that's the
    /// area's border. False if neither edge can move.
    pub fn grow(&mut self, w: &W, axis: Axis, delta: i32, area: Rect, gaps: &Gaps) -> bool {
        let (far, near) = match axis {
            Axis::Horizontal => (Dir::Right, Dir::Left),
            Axis::Vertical => (Dir::Down, Dir::Up),
        };
        self.resize_edge(w, far, delta, area, gaps) || self.resize_edge(w, near, delta, area, gaps)
    }

    /// Dwindle: flips the direction of the split `w` sits in. False for
    /// master, or a lone tile.
    pub fn toggle_split(&mut self, w: &W, area: Rect, gaps: &Gaps) -> bool {
        match self {
            Tiling::Dwindle(d) => d.toggle_split(w, area, gaps),
            Tiling::Master(_) => false,
        }
    }

    /// Master: makes `w` the master (the old master takes its place); on
    /// the master itself, swaps it with the top of the stack. False for
    /// dwindle, or nothing to swap with.
    pub fn make_master(&mut self, w: &W) -> bool {
        match self {
            Tiling::Master(m) => m.make_master(w),
            Tiling::Dwindle(_) => false,
        }
    }
}

/// The tile beside `w` in direction `dir`: the nearest one past that edge
/// that lines up with it, preferring the one sharing the longest stretch.
pub fn neighbour<W: Clone + PartialEq>(tiles: &[(W, Rect)], w: &W, dir: Dir) -> Option<W> {
    let (_, r) = tiles.iter().find(|(x, _)| x == w)?;
    tiles
        .iter()
        .filter(|(x, _)| x != w)
        .filter_map(|(x, c)| {
            let (distance, overlap) = match dir {
                Dir::Left => (r.x - c.right(), span(r.y, r.bottom(), c.y, c.bottom())),
                Dir::Right => (c.x - r.right(), span(r.y, r.bottom(), c.y, c.bottom())),
                Dir::Up => (r.y - c.bottom(), span(r.x, r.right(), c.x, c.right())),
                Dir::Down => (c.y - r.bottom(), span(r.x, r.right(), c.x, c.right())),
            };
            (distance >= 0 && overlap > 0).then_some((distance, -overlap, x))
        })
        .min_by_key(|(d, o, _)| (*d, *o))
        .map(|(_, _, x)| x.clone())
}

/// What dropping a dragged tile on another does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Drop {
    /// On its top band (the tab strip): it joins that tile's group.
    Group,
    /// On the middle: the two swap places.
    Swap,
    /// Near an edge: it goes on that side, splitting the tile.
    Side(Dir),
}

impl Drop {
    /// The part of `target` the dropped tile will take (for the preview).
    pub fn preview(self, target: Rect) -> Rect {
        match self {
            Drop::Group => Rect::new(target.x, target.y, target.w, TAB_STRIP.min(target.h)),
            Drop::Swap => target,
            Drop::Side(dir) => {
                let (a, b) = target.split(dir.axis(), 0.5, 0);
                if dir.forward() {
                    b
                } else {
                    a
                }
            }
        }
    }
}

/// Where on `target` the point `(x, y)` drops a tile: the top band (as
/// tall as a tab strip) groups, the middle third (each way) swaps,
/// elsewhere the nearest edge.
pub fn drop_zone(target: Rect, x: i32, y: i32) -> Drop {
    if y < target.y + TAB_STRIP {
        return Drop::Group;
    }
    let (fx, fy) = fractions(target, x, y);
    if (1.0 / 3.0..=2.0 / 3.0).contains(&fx) && (1.0 / 3.0..=2.0 / 3.0).contains(&fy) {
        return Drop::Swap;
    }
    Drop::Side(nearest_side(target, x, y))
}

/// The edge of `target` nearest `(x, y)` (the diagonals divide the sides).
pub fn nearest_side(target: Rect, x: i32, y: i32) -> Dir {
    let (fx, fy) = fractions(target, x, y);
    let edges = [(fx, Dir::Left), (1.0 - fx, Dir::Right), (fy, Dir::Up), (1.0 - fy, Dir::Down)];
    edges.iter().min_by(|a, b| a.0.total_cmp(&b.0)).map(|e| e.1).unwrap_or(Dir::Right)
}

/// How far across `target` the point is, each way (0..1).
fn fractions(target: Rect, x: i32, y: i32) -> (f64, f64) {
    ((x - target.x) as f64 / target.w.max(1) as f64, (y - target.y) as f64 / target.h.max(1) as f64)
}

/// How much [a0, a1) and [b0, b1) overlap.
fn span(a0: i32, a1: i32, b0: i32, b1: i32) -> i32 {
    a1.min(b1) - a0.max(b0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Rect = Rect::new(0, 0, 1000, 600);
    const NO_GAPS: Gaps = Gaps { inner: 0, outer: 0, smart: false };

    #[test]
    fn layout_names_round_trip_and_cycle() {
        for l in Layout::ALL {
            assert_eq!(Layout::parse(l.as_str()), Some(l));
        }
        assert_eq!(Layout::parse("spiral"), None);
        assert_eq!(Layout::Floating.next(), Layout::Dwindle);
        assert_eq!(Layout::Dwindle.next(), Layout::Master);
        assert_eq!(Layout::Master.next(), Layout::Floating);
    }

    #[test]
    fn monitor_beats_workspace_beats_global() {
        let mut s = Settings { global: Layout::Dwindle, ..Settings::default() };
        assert_eq!(s.layout_for(0, Some("DEL-1")), Layout::Dwindle);
        s.workspaces.insert(0, Layout::Master);
        assert_eq!(s.layout_for(0, Some("DEL-1")), Layout::Master);
        assert_eq!(s.layout_for(1, Some("DEL-1")), Layout::Dwindle, "other workspaces keep the default");
        s.monitors.insert("DEL-1".into(), Layout::Floating);
        assert_eq!(s.layout_for(0, Some("DEL-1")), Layout::Floating);
        assert_eq!(s.layout_for(0, Some("LG-2")), Layout::Master);
        assert_eq!(s.layout_for(0, None), Layout::Master, "unknown monitor id");
    }

    #[test]
    fn the_default_is_floating() {
        assert_eq!(Settings::default().layout_for(3, Some("x")), Layout::Floating);
        assert!(Tiling::<u32>::new(Layout::Floating, &Settings::default(), [1, 2]).is_none());
    }

    #[test]
    fn split_leaves_the_gap_and_fills_the_rest() {
        let (a, b) = Rect::new(10, 20, 101, 50).split(Axis::Horizontal, 0.5, 11);
        assert_eq!(a, Rect::new(10, 20, 45, 50));
        assert_eq!(b, Rect::new(66, 20, 45, 50));
        assert_eq!(b.right(), 111);
        let (a, b) = Rect::new(0, 0, 100, 100).split(Axis::Vertical, 0.25, 0);
        assert_eq!((a.h, b.y, b.h), (25, 25, 75));
        // A gap wider than the rectangle can't push the halves outside it.
        let (a, b) = Rect::new(0, 0, 4, 4).split(Axis::Horizontal, 0.5, 10);
        assert_eq!((a.w, b.w, b.right()), (0, 0, 4));
    }

    #[test]
    fn divide_hands_out_leftover_pixels() {
        let parts = Rect::new(0, 0, 100, 10).divide(Axis::Horizontal, 3, 5);
        let widths: Vec<_> = parts.iter().map(|r| r.w).collect();
        assert_eq!(widths, vec![30, 30, 30]);
        assert_eq!(parts[2].right(), 100);
        let parts = Rect::new(0, 0, 10, 101).divide(Axis::Vertical, 2, 0);
        assert_eq!((parts[0].h, parts[1].y, parts[1].h), (51, 51, 50));
        assert!(Rect::new(0, 0, 10, 10).divide(Axis::Vertical, 0, 4).is_empty());
    }

    #[test]
    fn smart_gaps_drop_gaps_for_a_lone_tile() {
        let g = Gaps { inner: 10, outer: 20, smart: true };
        assert_eq!(g.apply(AREA, 1), (AREA, 0));
        assert_eq!(g.apply(AREA, 2), (Rect::new(20, 20, 960, 560), 10));
        let g = Gaps { smart: false, ..g };
        assert_eq!(g.apply(AREA, 1), (Rect::new(20, 20, 960, 560), 10));
    }

    #[test]
    fn neighbour_picks_the_closest_lined_up_tile() {
        //  1 | 2
        //  --+---
        //  3 | 4 (4 is wider than 2)
        let tiles = vec![
            (1, Rect::new(0, 0, 500, 300)),
            (2, Rect::new(500, 0, 500, 300)),
            (3, Rect::new(0, 300, 400, 300)),
            (4, Rect::new(400, 300, 600, 300)),
        ];
        assert_eq!(neighbour(&tiles, &1, Dir::Right), Some(2));
        assert_eq!(neighbour(&tiles, &1, Dir::Down), Some(3), "3 shares more of 1's bottom edge");
        assert_eq!(neighbour(&tiles, &2, Dir::Down), Some(4));
        assert_eq!(neighbour(&tiles, &4, Dir::Left), Some(3));
        assert_eq!(neighbour(&tiles, &1, Dir::Left), None);
        assert_eq!(neighbour(&tiles, &1, Dir::Up), None);
        assert_eq!(neighbour(&tiles, &9, Dir::Up), None, "unknown window");
    }

    #[test]
    fn drops_swap_in_the_middle_and_split_near_an_edge() {
        let t = Rect::new(100, 100, 300, 300);
        assert_eq!(drop_zone(t, 250, 250), Drop::Swap);
        assert_eq!(drop_zone(t, 110, 250), Drop::Side(Dir::Left));
        assert_eq!(drop_zone(t, 390, 250), Drop::Side(Dir::Right));
        assert_eq!(drop_zone(t, 250, 140), Drop::Side(Dir::Up));
        assert_eq!(drop_zone(t, 250, 395), Drop::Side(Dir::Down));
        // Near a corner, the closer edge wins.
        assert_eq!(drop_zone(t, 110, 150), Drop::Side(Dir::Left));
        assert_eq!(drop_zone(t, 150, 135), Drop::Side(Dir::Up));
        // The top band groups.
        assert_eq!(drop_zone(t, 250, 110), Drop::Group);
        assert_eq!(Drop::Group.preview(t), Rect::new(100, 100, 300, TAB_STRIP));
        assert_eq!(nearest_side(t, 250, 250), Dir::Left, "a tie in the middle picks the first");
        assert_eq!(Drop::Swap.preview(t), t);
        assert_eq!(Drop::Side(Dir::Right).preview(t), Rect::new(250, 100, 150, 300));
        assert_eq!(Drop::Side(Dir::Up).preview(t), Rect::new(100, 100, 300, 150));
    }

    #[test]
    fn move_dir_swaps_with_the_neighbour() {
        let mut t = Tiling::new(Layout::Dwindle, &Settings::default(), [1, 2]).unwrap();
        assert!(t.move_dir(&1, Dir::Right, AREA, &NO_GAPS));
        assert_eq!(t.windows(), vec![2, 1]);
        assert!(!t.move_dir(&1, Dir::Right, AREA, &NO_GAPS), "nothing further right");
    }

    #[test]
    fn cycling_layouts_keeps_the_windows_in_order() {
        let s = Settings::default();
        let d = Tiling::new(Layout::Dwindle, &s, [1, 2, 3]).unwrap();
        let m = Tiling::new(Layout::Master, &s, d.windows()).unwrap();
        assert_eq!(m.windows(), vec![1, 2, 3]);
        assert_eq!(m.layout(), Layout::Master);
        let d = Tiling::new(Layout::Dwindle, &s, m.windows()).unwrap();
        assert_eq!(d.windows(), vec![1, 2, 3]);
    }

    #[test]
    fn insert_remove_and_swap_ignore_bad_input() {
        let mut t = Tiling::new(Layout::Master, &Settings::default(), [1, 2]).unwrap();
        t.insert(1, None);
        t.insert_at(2, &1, Dir::Left);
        assert_eq!(t.windows(), vec![1, 2], "already tiled");
        assert!(!t.remove(&7));
        assert!(!t.swap(&1, &7));
        assert!(!t.swap(&1, &1));
        assert!(t.swap(&1, &2));
        assert_eq!(t.windows(), vec![2, 1]);
        assert!(!t.toggle_split(&1, AREA, &NO_GAPS), "master has no splits to flip");
    }

    #[test]
    fn replace_takes_the_place_of_a_tile() {
        for layout in [Layout::Dwindle, Layout::Master] {
            let mut t = Tiling::new(layout, &Settings::default(), [1, 2, 3]).unwrap();
            assert!(t.replace(&2, 9));
            assert_eq!(t.windows(), vec![1, 9, 3]);
            assert!(!t.replace(&2, 8), "2 is gone");
            assert!(!t.replace(&1, 3), "3 is already tiled");
        }
    }

    #[test]
    fn grow_falls_back_to_the_near_edge() {
        let mut t = Tiling::new(Layout::Dwindle, &Settings::default(), [1, 2]).unwrap();
        // 2 is on the right: its right edge is the border, so its left one moves.
        assert!(t.grow(&2, Axis::Horizontal, 100, AREA, &NO_GAPS));
        let tiles = t.arrange(AREA, &NO_GAPS);
        assert_eq!(tiles[1], (2, Rect::new(400, 0, 600, 600)));
        assert!(!t.grow(&2, Axis::Vertical, 100, AREA, &NO_GAPS), "nothing above or below");
    }

    #[test]
    fn apply_settings_updates_master_and_dwindle() {
        let s = Settings::default();
        let mut m = Tiling::new(Layout::Master, &s, [1, 2]).unwrap();
        m.apply_settings(&Settings { master_ratio: 0.5, master_side: Side::Right, ..s.clone() });
        assert_eq!(m.arrange(AREA, &NO_GAPS)[0], (1, Rect::new(500, 0, 500, 600)));
        let mut d = Tiling::new(Layout::Dwindle, &s, [1]).unwrap();
        d.apply_settings(&Settings { dwindle_split: Some(Axis::Vertical), ..s });
        d.insert(2, Some(&1));
        assert_eq!(d.arrange(AREA, &NO_GAPS)[1], (2, Rect::new(0, 300, 1000, 300)));
    }
}
