//! Snapping on floating workspaces: a window dragged to an edge or corner
//! fills a half or a quarter, a picker or a zone grid puts it in a cell,
//! and SUPER+SHIFT+arrows step it between snaps.
//!
//! A snap is a [`Zone`]: fractions of the output's usable area, not pixels,
//! so it survives the bar and the outputs changing. Like the rest of
//! `layout/`, nothing here knows about Smithay.

use super::{Axis, Dir, Gaps, Rect};

/// The smallest zone, as a fraction of the area on each axis.
pub const MIN_ZONE: f64 = 0.1;
/// The most zones a grid can have.
pub const MAX_ZONES: usize = 16;
/// How close (px) the pointer must get to an edge to snap.
pub const EDGE: f64 = 4.0;
/// How far (px) along an edge from a corner still counts as that corner.
pub const CORNER: f64 = 64.0;

/// Fractions closer than this are the same.
const EPS: f64 = 1e-6;

fn same(a: f64, b: f64) -> bool {
    (a - b).abs() < EPS
}

/// A part of the usable area, as fractions of it (0–1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Zone {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Zone {
    pub const FULL: Zone = Zone::new(0.0, 0.0, 1.0, 1.0);

    pub const fn new(x: f64, y: f64, w: f64, h: f64) -> Zone {
        Zone { x, y, w, h }
    }

    pub fn right(&self) -> f64 {
        self.x + self.w
    }

    pub fn bottom(&self) -> f64 {
        self.y + self.h
    }

    /// The left or right half (Up and Down give the top or bottom half).
    pub fn half(side: Dir) -> Zone {
        match side {
            Dir::Left => Zone::new(0.0, 0.0, 0.5, 1.0),
            Dir::Right => Zone::new(0.5, 0.0, 0.5, 1.0),
            Dir::Up => Zone::new(0.0, 0.0, 1.0, 0.5),
            Dir::Down => Zone::new(0.0, 0.5, 1.0, 0.5),
        }
    }

    /// The quarter on side `h` (left or right) and `v` (up or down).
    pub fn quarter(h: Dir, v: Dir) -> Zone {
        let x = if h == Dir::Right { 0.5 } else { 0.0 };
        let y = if v == Dir::Down { 0.5 } else { 0.0 };
        Zone::new(x, y, 0.5, 0.5)
    }

    /// The same zone flipped left to right.
    pub fn mirrored(&self) -> Zone {
        Zone::new(1.0 - self.right(), self.y, self.w, self.h)
    }

    pub fn same_as(&self, o: &Zone) -> bool {
        same(self.x, o.x) && same(self.y, o.y) && same(self.w, o.w) && same(self.h, o.h)
    }

    fn start(&self, axis: Axis) -> f64 {
        match axis {
            Axis::Horizontal => self.x,
            Axis::Vertical => self.y,
        }
    }

    fn end(&self, axis: Axis) -> f64 {
        match axis {
            Axis::Horizontal => self.right(),
            Axis::Vertical => self.bottom(),
        }
    }

    /// Whether it reaches the area's edge on side `d`.
    fn touches(&self, d: Dir) -> bool {
        match d {
            Dir::Left => same(self.x, 0.0),
            Dir::Up => same(self.y, 0.0),
            Dir::Right => same(self.right(), 1.0),
            Dir::Down => same(self.bottom(), 1.0),
        }
    }

    /// The zone's rectangle in `area`, with the gaps tiles have: `outer`
    /// around the area, `inner` between zones that meet.
    pub fn rect(&self, area: Rect, gaps: &Gaps) -> Rect {
        let inner = gaps.inner.max(0);
        let c = area.shrink(gaps.outer.max(0));
        let at = |start: i32, len: i32, f: f64| start + (len as f64 * f).round() as i32;
        // A side inside the area gives up half the gap; the two halves of
        // a split add up to exactly `inner`.
        let (mut x0, mut x1) = (at(c.x, c.w, self.x), at(c.x, c.w, self.right()));
        let (mut y0, mut y1) = (at(c.y, c.h, self.y), at(c.y, c.h, self.bottom()));
        if !self.touches(Dir::Left) {
            x0 += inner / 2;
        }
        if !self.touches(Dir::Right) {
            x1 -= inner - inner / 2;
        }
        if !self.touches(Dir::Up) {
            y0 += inner / 2;
        }
        if !self.touches(Dir::Down) {
            y1 -= inner - inner / 2;
        }
        Rect::new(x0, y0, (x1 - x0).max(0), (y1 - y0).max(0))
    }

    /// Whether the point at fractions (`fx`, `fy`) is in it. The right and
    /// bottom edges belong to the next zone, except at the area's edge.
    fn contains(&self, fx: f64, fy: f64) -> bool {
        let in_x = fx >= self.x - EPS && (fx < self.right() - EPS || self.touches(Dir::Right));
        let in_y = fy >= self.y - EPS && (fy < self.bottom() - EPS || self.touches(Dir::Down));
        in_x && in_y
    }
}

/// A pointer coordinate `at` as a fraction of `area` along `axis`,
/// measured inside the outer gap as [`Zone::rect`] does (for edge drags).
pub fn fraction_at(area: Rect, gaps: &Gaps, axis: Axis, at: f64) -> f64 {
    let c = area.shrink(gaps.outer.max(0));
    let (start, len) = match axis {
        Axis::Horizontal => (c.x, c.w),
        Axis::Vertical => (c.y, c.h),
    };
    if len <= 0 {
        return 0.0;
    }
    ((at - start as f64) / len as f64).clamp(0.0, 1.0)
}

/// What a drag to an edge does.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Target {
    Zone(Zone),
    Maximize,
    /// The top edge's middle third: show the picker's cells to drop on.
    Picker,
}

/// The sides of the area where the pointer can't go any further (no other
/// monitor beyond). Only those snap, so windows still cross to a monitor
/// that's next to this one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Open {
    pub left: bool,
    pub right: bool,
    pub top: bool,
    pub bottom: bool,
}

impl Open {
    pub const ALL: Open = Open { left: true, right: true, top: true, bottom: true };
}

/// What dragging with the pointer at (`x`, `y`) snaps to, on the output
/// whose usable area is `area`. The pointer may be above the area (on the
/// bar). Left and right edges give halves; near a corner, quarters; the
/// top edge maximizes, except its middle third (the picker); the bottom
/// edge does nothing except at its corners.
pub fn edge_target(area: Rect, open: Open, x: f64, y: f64) -> Option<Target> {
    let (l, r, t, b) = (area.x as f64, area.right() as f64 - 1.0, area.y as f64, area.bottom() as f64 - 1.0);
    let at_l = open.left && x <= l + EDGE;
    let at_r = open.right && x >= r - EDGE;
    let at_t = open.top && y <= t + EDGE;
    let at_b = open.bottom && y >= b - EDGE;
    let near_v = if y <= t + CORNER {
        Some(Dir::Up)
    } else if y >= b - CORNER {
        Some(Dir::Down)
    } else {
        None
    };
    let near_h = if x <= l + CORNER {
        Some(Dir::Left)
    } else if x >= r - CORNER {
        Some(Dir::Right)
    } else {
        None
    };
    if at_l || at_r {
        let side = if at_l { Dir::Left } else { Dir::Right };
        return Some(Target::Zone(match near_v {
            Some(v) => Zone::quarter(side, v),
            None => Zone::half(side),
        }));
    }
    if at_t || at_b {
        let v = if at_t { Dir::Up } else { Dir::Down };
        if let Some(h) = near_h {
            return Some(Target::Zone(Zone::quarter(h, v)));
        }
        if at_t {
            let third = area.w as f64 / 3.0;
            let middle = x >= l + third && x < l + 2.0 * third;
            return Some(if middle { Target::Picker } else { Target::Maximize });
        }
    }
    None
}

/// The zone under the pointer at (`x`, `y`) in `area`, by index. Zones
/// are matched on fractions of the whole area, so the gaps between them
/// aren't dead space.
pub fn zone_at(zones: &[Zone], area: Rect, x: f64, y: f64) -> Option<usize> {
    if area.w <= 0 || area.h <= 0 {
        return None;
    }
    let fx = (x - area.x as f64) / area.w as f64;
    let fy = (y - area.y as f64) / area.h as f64;
    if !(0.0..=1.0).contains(&fx) || !(0.0..=1.0).contains(&fy) {
        return None;
    }
    zones.iter().position(|z| z.contains(fx, fy))
}

/// One of the picker's layouts: a name and its cells.
#[derive(Debug, Clone, PartialEq)]
pub struct PickerLayout {
    /// `halves`, `thirds`, `quarters`, `two-thirds-left`,
    /// `two-thirds-right`, or `zones` (the monitor's own grid).
    pub id: &'static str,
    pub cells: Vec<Zone>,
}

/// The grid used when a monitor has none: two thirds and a third.
pub fn default_grid() -> Vec<Zone> {
    vec![Zone::new(0.0, 0.0, 2.0 / 3.0, 1.0), Zone::new(2.0 / 3.0, 0.0, 1.0 / 3.0, 1.0)]
}

/// The picker's layouts, with the monitor's own zone grid last if it has
/// one.
pub fn picker_layouts(grid: Option<&[Zone]>) -> Vec<PickerLayout> {
    let third = 1.0 / 3.0;
    let mut layouts = vec![
        PickerLayout { id: "halves", cells: vec![Zone::half(Dir::Left), Zone::half(Dir::Right)] },
        PickerLayout { id: "thirds", cells: (0..3).map(|i| Zone::new(i as f64 * third, 0.0, third, 1.0)).collect() },
        PickerLayout {
            id: "quarters",
            cells: vec![
                Zone::quarter(Dir::Left, Dir::Up),
                Zone::quarter(Dir::Right, Dir::Up),
                Zone::quarter(Dir::Left, Dir::Down),
                Zone::quarter(Dir::Right, Dir::Down),
            ],
        },
        PickerLayout { id: "two-thirds-left", cells: default_grid() },
        PickerLayout {
            id: "two-thirds-right",
            cells: vec![Zone::new(0.0, 0.0, third, 1.0), Zone::new(third, 0.0, 2.0 * third, 1.0)],
        },
    ];
    if let Some(g) = grid.filter(|g| !g.is_empty()) {
        layouts.push(PickerLayout { id: "zones", cells: g.to_vec() });
    }
    layouts
}

/// The picker's measurements, from the Figma frames "Snap Picker - SUPER +
/// Z" and "Snap Picker - Maximize button" (the shell draws it at these).
pub mod picker {
    /// A layout's tile.
    pub const TILE_W: i32 = 132;
    pub const TILE_H: i32 = 80;
    pub const TILE_GAP: i32 = 14;
    /// Around the tiles; the top holds the title row.
    pub const PAD_X: i32 = 24;
    pub const PAD_TOP: i32 = 52;
    pub const PAD_BOTTOM: i32 = 20;
    /// Cells inside a tile: in from its edge, and apart.
    pub const INSET: i32 = 6;
    pub const CELL_GAP: i32 = 4;
}

/// Where the picker's tiles and cells are, relative to its top left.
#[derive(Debug, Clone, PartialEq)]
pub struct PickerGeometry {
    pub width: i32,
    pub height: i32,
    /// Each layout's tile, and its cells' rectangles.
    pub tiles: Vec<(Rect, Vec<Rect>)>,
}

impl PickerGeometry {
    pub fn new(layouts: &[PickerLayout]) -> PickerGeometry {
        use picker::*;
        let n = layouts.len() as i32;
        let width = 2 * PAD_X + n * TILE_W + (n - 1).max(0) * TILE_GAP;
        let gaps = Gaps { inner: CELL_GAP, outer: INSET, smart: false };
        let tiles = layouts
            .iter()
            .enumerate()
            .map(|(i, l)| {
                let tile = Rect::new(PAD_X + i as i32 * (TILE_W + TILE_GAP), PAD_TOP, TILE_W, TILE_H);
                (tile, l.cells.iter().map(|z| z.rect(tile, &gaps)).collect())
            })
            .collect();
        PickerGeometry { width, height: PAD_TOP + TILE_H + PAD_BOTTOM, tiles }
    }

    /// The (layout, cell) under the point (`x`, `y`), relative to the
    /// picker. Anywhere on a tile counts, the gaps between its cells too.
    pub fn hit(&self, layouts: &[PickerLayout], x: f64, y: f64) -> Option<(usize, usize)> {
        self.tiles.iter().zip(layouts).enumerate().find_map(|(i, ((tile, _), l))| {
            let inside =
                x >= tile.x as f64 && x < tile.right() as f64 && y >= tile.y as f64 && y < tile.bottom() as f64;
            if !inside {
                return None;
            }
            zone_at(&l.cells, *tile, x, y).map(|c| (i, c))
        })
    }
}

/// The cell the arrow `dir` moves to in the picker: left and right go
/// through every cell in order, across the layouts; up and down move
/// within a layout (quarters). Nothing chosen yet: the first cell.
pub fn picker_step(geometry: &PickerGeometry, at: Option<(usize, usize)>, dir: Dir) -> Option<(usize, usize)> {
    let cells: Vec<(usize, usize)> =
        geometry.tiles.iter().enumerate().flat_map(|(l, (_, cs))| (0..cs.len()).map(move |c| (l, c))).collect();
    let Some(at) = at else { return cells.first().copied() };
    let i = cells.iter().position(|c| *c == at)?;
    match dir {
        Dir::Left => Some(cells[i.saturating_sub(1)]),
        Dir::Right => Some(cells[(i + 1).min(cells.len() - 1)]),
        Dir::Up | Dir::Down => {
            let rects: Vec<(usize, Rect)> = geometry.tiles[at.0].1.iter().copied().enumerate().collect();
            Some(super::neighbour(&rects, &at.1, dir).map_or(at, |c| (at.0, c)))
        }
    }
}

/// A zone grid from the settings (`[[x, y, w, h], ...]`), or None if it's
/// unusable: empty, too many zones, a zone outside the area or smaller than
/// [`MIN_ZONE`], or two zones overlapping (a point must have one zone).
pub fn validate_grid(raw: &[[f64; 4]]) -> Option<Vec<Zone>> {
    if raw.is_empty() || raw.len() > MAX_ZONES {
        return None;
    }
    let zones: Vec<Zone> = raw.iter().map(|[x, y, w, h]| Zone::new(*x, *y, *w, *h)).collect();
    for z in &zones {
        let ok = [z.x, z.y, z.w, z.h].iter().all(|v| v.is_finite())
            && z.x >= -EPS
            && z.y >= -EPS
            && z.w >= MIN_ZONE - EPS
            && z.h >= MIN_ZONE - EPS
            && z.right() <= 1.0 + EPS
            && z.bottom() <= 1.0 + EPS;
        if !ok {
            return None;
        }
    }
    for (i, a) in zones.iter().enumerate() {
        for b in &zones[i + 1..] {
            let w = a.right().min(b.right()) - a.x.max(b.x);
            let h = a.bottom().min(b.bottom()) - a.y.max(b.y);
            if w > EPS && h > EPS {
                return None;
            }
        }
    }
    Some(zones)
}

/// Dragging the edge of a snapped window, and with it the edges of the
/// other snapped windows on the same line, so they resize together.
#[derive(Debug, Clone, PartialEq)]
pub struct EdgeDrag<W> {
    /// Horizontal: the line is upright and moves left and right.
    pub axis: Axis,
    /// Where the line is, as a fraction of the area.
    pub line: f64,
    /// The windows moving with it, and whether it's their far edge (right
    /// or bottom) that's on the line.
    members: Vec<(W, bool)>,
}

impl<W: Clone + PartialEq> EdgeDrag<W> {
    /// Dragging `w`'s edge on side `dir`, among the windows snapped on one
    /// output. Every snapped window with an edge on that line moves with
    /// it, as long as they line up with `w` or with each other along it
    /// (so dragging the middle of four quarters moves all four). None if
    /// `w` isn't snapped or that edge is the area's own.
    pub fn new(snapped: &[(W, Zone)], w: &W, dir: Dir) -> Option<EdgeDrag<W>> {
        let (_, z) = snapped.iter().find(|(x, _)| x == w)?;
        let axis = dir.axis();
        let line = if dir.forward() { z.end(axis) } else { z.start(axis) };
        if line <= EPS || line >= 1.0 - EPS {
            return None;
        }
        let across = axis.other();
        let on_line: Vec<(&W, bool, f64, f64)> = snapped
            .iter()
            .filter_map(|(x, z)| {
                let far = if same(z.end(axis), line) {
                    true
                } else if same(z.start(axis), line) {
                    false
                } else {
                    return None;
                };
                Some((x, far, z.start(across), z.end(across)))
            })
            .collect();
        let mut members = vec![(w.clone(), dir.forward())];
        let (mut lo, mut hi) = (z.start(across), z.end(across));
        loop {
            let before = members.len();
            for (x, far, s, e) in &on_line {
                if members.iter().any(|(m, _)| m == *x) {
                    continue;
                }
                if *s <= hi + EPS && *e >= lo - EPS {
                    members.push(((*x).clone(), *far));
                    lo = lo.min(*s);
                    hi = hi.max(*e);
                }
            }
            if members.len() == before {
                break;
            }
        }
        Some(EdgeDrag { axis, line, members })
    }

    /// The windows that move (the dragged one first).
    pub fn windows(&self) -> impl Iterator<Item = &W> {
        self.members.iter().map(|(w, _)| w)
    }

    /// Moves the line to the fraction `to`, but only as far as keeps every
    /// window at least [`MIN_ZONE`]. Updates their zones in `snapped` and
    /// returns where the line ended up.
    pub fn drag(&mut self, snapped: &mut [(W, Zone)], to: f64) -> f64 {
        let axis = self.axis;
        let (mut lo, mut hi) = (MIN_ZONE, 1.0 - MIN_ZONE);
        for (w, far) in &self.members {
            if let Some((_, z)) = snapped.iter().find(|(x, _)| x == w) {
                if *far {
                    lo = lo.max(z.start(axis) + MIN_ZONE);
                } else {
                    hi = hi.min(z.end(axis) - MIN_ZONE);
                }
            }
        }
        if lo > hi {
            return self.line;
        }
        let to = to.clamp(lo, hi);
        for (w, far) in &self.members {
            if let Some((_, z)) = snapped.iter_mut().find(|(x, _)| x == w) {
                let (start, size) = match axis {
                    Axis::Horizontal => (&mut z.x, &mut z.w),
                    Axis::Vertical => (&mut z.y, &mut z.h),
                };
                if *far {
                    *size = to - *start;
                } else {
                    *size = *start + *size - to;
                    *start = to;
                }
            }
        }
        self.line = to;
        to
    }
}

/// Where a window is, for SUPER+SHIFT+arrows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeyState {
    Free,
    Snapped(Zone),
    Maximized,
}

/// What SUPER+SHIFT+arrow does next.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum KeyStep {
    Snap(Zone),
    Maximize,
    /// Back to the window's own size and place.
    Restore,
    /// Snap into this zone on the next monitor in this direction (if there
    /// is none, nothing happens).
    NextOutput(Dir, Zone),
    Nothing,
}

/// A half or a quarter: its side, and for a quarter its top or bottom.
fn half_or_quarter(z: &Zone) -> Option<(Dir, Option<Dir>)> {
    if !same(z.w, 0.5) {
        return None;
    }
    let h = if z.touches(Dir::Left) { Dir::Left } else { Dir::Right };
    if same(z.h, 1.0) {
        Some((h, None))
    } else if same(z.h, 0.5) {
        Some((h, Some(if z.touches(Dir::Up) { Dir::Up } else { Dir::Down })))
    } else {
        None
    }
}

/// The next snap for SUPER+SHIFT+`dir`, as Win+arrows do:
/// - Left or right snaps to that half. Pressed again towards the side the
///   window already reaches, it goes on to the next monitor (the mirrored
///   zone); towards the other side, it gives back the window's own size.
/// - Up maximizes; from a half it's the top quarter, from a bottom quarter
///   the half, from a top quarter maximize.
/// - Down from maximized gives the size back; from a half it's the bottom
///   quarter, from a top quarter the half, from a bottom quarter the size
///   back. Down never minimises.
pub fn key_step(state: KeyState, dir: Dir) -> KeyStep {
    match (state, dir) {
        (KeyState::Free | KeyState::Maximized, Dir::Left | Dir::Right) => KeyStep::Snap(Zone::half(dir)),
        (KeyState::Free, Dir::Up) => KeyStep::Maximize,
        (KeyState::Free, Dir::Down) | (KeyState::Maximized, Dir::Up) => KeyStep::Nothing,
        (KeyState::Maximized, Dir::Down) => KeyStep::Restore,
        (KeyState::Snapped(z), Dir::Left | Dir::Right) => {
            let full_width = z.touches(Dir::Left) && z.touches(Dir::Right);
            if full_width {
                KeyStep::Snap(Zone::half(dir))
            } else if z.touches(dir) {
                KeyStep::NextOutput(dir, z.mirrored())
            } else if z.touches(dir.opposite()) {
                KeyStep::Restore
            } else {
                KeyStep::Snap(Zone::half(dir))
            }
        }
        (KeyState::Snapped(z), Dir::Up | Dir::Down) => match half_or_quarter(&z) {
            Some((h, None)) => KeyStep::Snap(Zone::quarter(h, dir)),
            Some((_, Some(v))) if v == dir => {
                if dir == Dir::Up {
                    KeyStep::Maximize
                } else {
                    KeyStep::Restore
                }
            }
            Some((h, Some(_))) => KeyStep::Snap(Zone::half(h)),
            None if dir == Dir::Up => KeyStep::Maximize,
            None => KeyStep::Restore,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Rect = Rect::new(0, 40, 1000, 600);
    const NO_GAPS: Gaps = Gaps { inner: 0, outer: 0, smart: false };
    const GAPS: Gaps = Gaps { inner: 10, outer: 20, smart: false };

    fn target(x: f64, y: f64) -> Option<Target> {
        edge_target(AREA, Open::ALL, x, y)
    }

    #[test]
    fn zones_become_rectangles_with_the_tiles_gaps() {
        assert_eq!(Zone::FULL.rect(AREA, &NO_GAPS), AREA);
        assert_eq!(Zone::half(Dir::Left).rect(AREA, &NO_GAPS), Rect::new(0, 40, 500, 600));
        // Outer gap around the area, exactly `inner` between the halves.
        let l = Zone::half(Dir::Left).rect(AREA, &GAPS);
        let r = Zone::half(Dir::Right).rect(AREA, &GAPS);
        assert_eq!(l, Rect::new(20, 60, 475, 560));
        assert_eq!(r, Rect::new(505, 60, 475, 560));
        assert_eq!(r.x - l.right(), 10);
        // Odd gaps still add up.
        let odd = Gaps { inner: 7, outer: 0, smart: false };
        let t = Zone::quarter(Dir::Left, Dir::Up).rect(AREA, &odd);
        let b = Zone::quarter(Dir::Left, Dir::Down).rect(AREA, &odd);
        assert_eq!(b.y - t.bottom(), 7);
        // Thirds fill the area.
        let third = 1.0 / 3.0;
        let last = Zone::new(2.0 * third, 0.0, third, 1.0).rect(AREA, &NO_GAPS);
        assert_eq!(last.right(), AREA.right());
    }

    #[test]
    fn a_pointer_position_becomes_a_fraction() {
        assert_eq!(fraction_at(AREA, &NO_GAPS, Axis::Horizontal, 250.0), 0.25);
        assert_eq!(fraction_at(AREA, &GAPS, Axis::Horizontal, 500.0), 0.5);
        assert_eq!(fraction_at(AREA, &NO_GAPS, Axis::Vertical, 340.0), 0.5);
        assert_eq!(fraction_at(AREA, &NO_GAPS, Axis::Horizontal, -50.0), 0.0);
        assert_eq!(fraction_at(AREA, &NO_GAPS, Axis::Horizontal, 5000.0), 1.0);
        assert_eq!(fraction_at(Rect::new(0, 0, 0, 0), &NO_GAPS, Axis::Horizontal, 5.0), 0.0);
    }

    #[test]
    fn side_edges_snap_halves_and_corners_quarters() {
        assert_eq!(target(0.0, 340.0), Some(Target::Zone(Zone::half(Dir::Left))));
        assert_eq!(target(999.0, 340.0), Some(Target::Zone(Zone::half(Dir::Right))));
        assert_eq!(target(2.0, 50.0), Some(Target::Zone(Zone::quarter(Dir::Left, Dir::Up))));
        assert_eq!(target(999.0, 630.0), Some(Target::Zone(Zone::quarter(Dir::Right, Dir::Down))));
        // Along the top or bottom near a corner too.
        assert_eq!(target(30.0, 40.0), Some(Target::Zone(Zone::quarter(Dir::Left, Dir::Up))));
        assert_eq!(target(980.0, 639.0), Some(Target::Zone(Zone::quarter(Dir::Right, Dir::Down))));
        // Away from the edges: nothing.
        assert_eq!(target(500.0, 340.0), None);
        assert_eq!(target(10.0, 340.0), None);
    }

    #[test]
    fn the_top_edge_maximizes_except_its_middle_third() {
        assert_eq!(target(200.0, 40.0), Some(Target::Maximize));
        assert_eq!(target(800.0, 40.0), Some(Target::Maximize));
        assert_eq!(target(500.0, 40.0), Some(Target::Picker));
        // On the bar, above the usable area, counts as the top.
        assert_eq!(target(500.0, 5.0), Some(Target::Picker));
        // The bottom edge does nothing away from its corners.
        assert_eq!(target(500.0, 639.0), None);
    }

    #[test]
    fn edges_with_a_monitor_beyond_dont_snap() {
        let open = Open { right: false, ..Open::ALL };
        assert_eq!(edge_target(AREA, open, 999.0, 340.0), None);
        // Its corners still snap from the top edge.
        assert_eq!(edge_target(AREA, open, 980.0, 40.0), Some(Target::Zone(Zone::quarter(Dir::Right, Dir::Up))));
        assert_eq!(edge_target(AREA, Open { top: false, ..Open::ALL }, 200.0, 40.0), None);
    }

    #[test]
    fn the_zone_under_the_pointer() {
        let zones = default_grid();
        assert_eq!(zone_at(&zones, AREA, 100.0, 300.0), Some(0));
        assert_eq!(zone_at(&zones, AREA, 900.0, 300.0), Some(1));
        // The very right and bottom edges belong to the last zones.
        assert_eq!(zone_at(&zones, AREA, 1000.0, 640.0), Some(1));
        assert_eq!(zone_at(&zones, AREA, 1100.0, 300.0), None);
        // A gap in the grid has no zone.
        let gappy = vec![Zone::new(0.0, 0.0, 0.4, 1.0), Zone::new(0.6, 0.0, 0.4, 1.0)];
        assert_eq!(zone_at(&gappy, AREA, 500.0, 300.0), None);
        assert_eq!(zone_at(&zones, Rect::new(0, 0, 0, 0), 0.0, 0.0), None);
    }

    #[test]
    fn picker_layouts_fill_the_area_and_add_the_monitors_grid() {
        let layouts = picker_layouts(None);
        let ids: Vec<_> = layouts.iter().map(|l| l.id).collect();
        assert_eq!(ids, ["halves", "thirds", "quarters", "two-thirds-left", "two-thirds-right"]);
        for l in &layouts {
            let area: f64 = l.cells.iter().map(|z| z.w * z.h).sum();
            assert!(same(area, 1.0), "{} covers the area", l.id);
            assert!(validate_grid(&l.cells.iter().map(|z| [z.x, z.y, z.w, z.h]).collect::<Vec<_>>()).is_some());
        }
        let grid = [Zone::half(Dir::Up), Zone::half(Dir::Down)];
        let with = picker_layouts(Some(&grid));
        assert_eq!(with.last().map(|l| l.id), Some("zones"));
        assert_eq!(with.last().map(|l| l.cells.len()), Some(2));
        assert_eq!(picker_layouts(Some(&[])).len(), 5);
    }

    #[test]
    fn grids_are_validated() {
        assert_eq!(validate_grid(&[[0.0, 0.0, 0.5, 1.0], [0.5, 0.0, 0.5, 1.0]]).map(|g| g.len()), Some(2));
        assert!(validate_grid(&[]).is_none(), "empty");
        assert!(validate_grid(&[[0.0, 0.0, 0.6, 1.0], [0.5, 0.0, 0.5, 1.0]]).is_none(), "overlap");
        assert!(validate_grid(&[[0.6, 0.0, 0.5, 1.0]]).is_none(), "outside");
        assert!(validate_grid(&[[-0.1, 0.0, 0.5, 1.0]]).is_none(), "outside");
        assert!(validate_grid(&[[0.0, 0.0, 0.05, 1.0]]).is_none(), "too small");
        assert!(validate_grid(&[[0.0, 0.0, f64::NAN, 1.0]]).is_none(), "not a number");
        let many: Vec<[f64; 4]> = (0..17).map(|i| [0.0, 0.0, 0.1, 0.1 + i as f64]).collect();
        assert!(validate_grid(&many).is_none(), "too many");
        // Gaps in a grid are fine.
        assert!(validate_grid(&[[0.0, 0.0, 0.4, 1.0], [0.6, 0.0, 0.4, 1.0]]).is_some());
    }

    #[test]
    fn dragging_a_shared_edge_resizes_both_halves() {
        let mut snapped = vec![(1, Zone::half(Dir::Left)), (2, Zone::half(Dir::Right))];
        let mut drag = EdgeDrag::new(&snapped, &1, Dir::Right).unwrap();
        assert_eq!(drag.axis, Axis::Horizontal);
        assert_eq!(drag.windows().copied().collect::<Vec<_>>(), [1, 2]);
        assert_eq!(drag.drag(&mut snapped, 0.7), 0.7);
        assert!(snapped[0].1.same_as(&Zone::new(0.0, 0.0, 0.7, 1.0)));
        assert!(snapped[1].1.same_as(&Zone::new(0.7, 0.0, 0.3, 1.0)));
        // Never smaller than MIN_ZONE.
        assert!(same(drag.drag(&mut snapped, 0.99), 1.0 - MIN_ZONE));
        assert!(same(snapped[1].1.w, MIN_ZONE));
        // The same line from the other window's side.
        let drag = EdgeDrag::new(&snapped, &2, Dir::Left).unwrap();
        assert_eq!(drag.windows().count(), 2);
    }

    #[test]
    fn the_middle_of_four_quarters_moves_all_four() {
        use Dir::*;
        let mut snapped = vec![
            (1, Zone::quarter(Left, Up)),
            (2, Zone::quarter(Right, Up)),
            (3, Zone::quarter(Left, Down)),
            (4, Zone::quarter(Right, Down)),
        ];
        let mut drag = EdgeDrag::new(&snapped, &1, Right).unwrap();
        let mut ws: Vec<_> = drag.windows().copied().collect();
        ws.sort();
        assert_eq!(ws, [1, 2, 3, 4]);
        drag.drag(&mut snapped, 0.4);
        assert!(same(snapped[2].1.w, 0.4) && same(snapped[3].1.x, 0.4));
        // The horizontal line: all four too.
        assert_eq!(EdgeDrag::new(&snapped, &1, Down).unwrap().windows().count(), 4);
    }

    #[test]
    fn only_windows_on_the_line_and_lined_up_move() {
        // A left half, and a right window that doesn't reach the line.
        let snapped = vec![(1, Zone::half(Dir::Left)), (2, Zone::new(0.6, 0.0, 0.4, 1.0))];
        let drag = EdgeDrag::new(&snapped, &1, Dir::Right).unwrap();
        assert_eq!(drag.windows().copied().collect::<Vec<_>>(), [1]);
        // On the line but not lined up with anything that moves.
        let snapped = vec![
            (1, Zone::new(0.0, 0.0, 0.5, 0.3)),
            (2, Zone::new(0.5, 0.0, 0.5, 0.3)),
            (3, Zone::new(0.5, 0.6, 0.5, 0.4)),
        ];
        let drag = EdgeDrag::new(&snapped, &1, Dir::Right).unwrap();
        assert_eq!(drag.windows().copied().collect::<Vec<_>>(), [1, 2]);
        // The area's own edge isn't dragged, and unknown windows aren't.
        assert!(EdgeDrag::new(&snapped, &1, Dir::Left).is_none());
        assert!(EdgeDrag::new(&snapped, &1, Dir::Up).is_none());
        assert!(EdgeDrag::new(&snapped, &9, Dir::Right).is_none());
    }

    #[test]
    fn arrows_from_a_free_or_maximized_window() {
        use Dir::*;
        assert_eq!(key_step(KeyState::Free, Left), KeyStep::Snap(Zone::half(Left)));
        assert_eq!(key_step(KeyState::Free, Right), KeyStep::Snap(Zone::half(Right)));
        assert_eq!(key_step(KeyState::Free, Up), KeyStep::Maximize);
        assert_eq!(key_step(KeyState::Free, Down), KeyStep::Nothing, "never minimises");
        assert_eq!(key_step(KeyState::Maximized, Down), KeyStep::Restore);
        assert_eq!(key_step(KeyState::Maximized, Up), KeyStep::Nothing);
        assert_eq!(key_step(KeyState::Maximized, Left), KeyStep::Snap(Zone::half(Left)));
    }

    #[test]
    fn arrows_from_halves_and_quarters() {
        use Dir::*;
        let left = KeyState::Snapped(Zone::half(Left));
        assert_eq!(key_step(left, Left), KeyStep::NextOutput(Left, Zone::half(Right)));
        assert_eq!(key_step(left, Right), KeyStep::Restore);
        assert_eq!(key_step(left, Up), KeyStep::Snap(Zone::quarter(Left, Up)));
        assert_eq!(key_step(left, Down), KeyStep::Snap(Zone::quarter(Left, Down)));
        let top_right = KeyState::Snapped(Zone::quarter(Right, Up));
        assert_eq!(key_step(top_right, Up), KeyStep::Maximize);
        assert_eq!(key_step(top_right, Down), KeyStep::Snap(Zone::half(Right)));
        assert_eq!(key_step(top_right, Right), KeyStep::NextOutput(Right, Zone::quarter(Left, Up)));
        assert_eq!(key_step(top_right, Left), KeyStep::Restore);
        let bottom_left = KeyState::Snapped(Zone::quarter(Left, Down));
        assert_eq!(key_step(bottom_left, Up), KeyStep::Snap(Zone::half(Left)));
        assert_eq!(key_step(bottom_left, Down), KeyStep::Restore);
    }

    #[test]
    fn arrows_from_other_zones() {
        use Dir::*;
        let middle = KeyState::Snapped(Zone::new(1.0 / 3.0, 0.0, 1.0 / 3.0, 1.0));
        assert_eq!(key_step(middle, Left), KeyStep::Snap(Zone::half(Left)));
        assert_eq!(key_step(middle, Up), KeyStep::Maximize);
        assert_eq!(key_step(middle, Down), KeyStep::Restore);
        let two_thirds = KeyState::Snapped(default_grid()[0]);
        let KeyStep::NextOutput(Left, z) = key_step(two_thirds, Left) else { panic!("goes to the next monitor") };
        assert!(z.same_as(&Zone::new(1.0 / 3.0, 0.0, 2.0 / 3.0, 1.0)));
        let top = KeyState::Snapped(Zone::half(Up));
        assert_eq!(key_step(top, Right), KeyStep::Snap(Zone::half(Right)));
    }

    #[test]
    fn the_picker_matches_the_figma_frame() {
        let layouts = picker_layouts(None);
        let g = PickerGeometry::new(&layouts);
        // Five tiles: the island's body in Figma is 764 x 152.
        assert_eq!((g.width, g.height), (764, 152));
        assert_eq!(g.tiles[1].0, Rect::new(170, 52, 132, 80));
        // Halves: 6px in, 4px apart.
        assert_eq!(g.tiles[0].1, vec![Rect::new(30, 58, 58, 68), Rect::new(92, 58, 58, 68)]);
        // With the monitor's own grid: a sixth tile.
        let grid = [Zone::half(Dir::Up), Zone::half(Dir::Down)];
        assert_eq!(PickerGeometry::new(&picker_layouts(Some(&grid))).width, 764 + 146);
    }

    #[test]
    fn the_picker_finds_the_cell_under_the_pointer() {
        let layouts = picker_layouts(None);
        let g = PickerGeometry::new(&layouts);
        assert_eq!(g.hit(&layouts, 40.0, 80.0), Some((0, 0)));
        assert_eq!(g.hit(&layouts, 100.0, 80.0), Some((0, 1)));
        // The gap between two cells of a tile belongs to one of them.
        assert_eq!(g.hit(&layouts, 89.5, 80.0).map(|h| h.0), Some(0));
        // Quarters' lower right.
        assert_eq!(g.hit(&layouts, 420.0, 120.0), Some((2, 3)));
        // Between tiles, above them, outside: nothing.
        assert_eq!(g.hit(&layouts, 160.0, 80.0), None);
        assert_eq!(g.hit(&layouts, 40.0, 20.0), None);
        assert_eq!(g.hit(&layouts, 2000.0, 80.0), None);
    }

    #[test]
    fn arrows_walk_the_picker() {
        use Dir::*;
        let layouts = picker_layouts(None);
        let g = PickerGeometry::new(&layouts);
        assert_eq!(picker_step(&g, None, Right), Some((0, 0)));
        assert_eq!(picker_step(&g, Some((0, 0)), Left), Some((0, 0)), "stays at the start");
        assert_eq!(picker_step(&g, Some((0, 1)), Right), Some((1, 0)), "on into the next layout");
        assert_eq!(picker_step(&g, Some((4, 1)), Right), Some((4, 1)), "stays at the end");
        // Quarters: down from the top left is the bottom left.
        assert_eq!(picker_step(&g, Some((2, 0)), Down), Some((2, 2)));
        assert_eq!(picker_step(&g, Some((2, 2)), Down), Some((2, 2)));
        assert_eq!(picker_step(&g, Some((0, 0)), Up), Some((0, 0)), "halves have nothing above");
    }
}
