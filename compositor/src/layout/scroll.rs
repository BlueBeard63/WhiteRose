//! Scrolling: a horizontal strip of columns, wider than the screen, that
//! scrolls to keep the focused column in view (niri's model).
//!
//! Each column holds one or more windows stacked top to bottom and has a
//! width as a fraction of the usable width, so a monitor or bar change
//! rescales it. The view offset is in pixels from the strip's left edge;
//! [`Scroll::arrange`] gives every window its rectangle, including the ones
//! scrolled off screen (the compositor hides those).

use super::{Dir, Gaps, Rect};

/// When moving focus scrolls the focused column to the middle
/// (`windows.scroll.center`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Center {
    /// Scroll just enough to show the column.
    Never,
    /// As `Never`, but centre the column when it doesn't fit on screen next
    /// to the one focused before it.
    #[default]
    Overflow,
    /// The focused column is always in the middle.
    Always,
}

impl Center {
    pub const ALL: [Center; 3] = [Center::Never, Center::Overflow, Center::Always];

    /// The name used in settings.
    pub fn as_str(self) -> &'static str {
        match self {
            Center::Never => "never",
            Center::Overflow => "overflow",
            Center::Always => "always",
        }
    }

    pub fn parse(s: &str) -> Option<Center> {
        Center::ALL.into_iter().find(|c| c.as_str() == s)
    }
}

/// The widths SUPER+W cycles through, as fractions of the usable width.
pub const WIDTHS: [f64; 3] = [1.0 / 3.0, 0.5, 2.0 / 3.0];
/// A new column's width.
pub const DEFAULT_WIDTH: f64 = 0.5;
/// The narrowest a column gets when resized with the mouse.
pub const MIN_WIDTH: f64 = 0.1;
/// The smallest share of its column a window gets when resized.
const MIN_HEIGHT: f64 = 0.1;

#[derive(Debug, Clone, PartialEq)]
struct Column<W> {
    /// Top to bottom.
    windows: Vec<W>,
    /// Each window's share of the column's height; they add up to 1.
    heights: Vec<f64>,
    /// A fraction of the usable width.
    width: f64,
    /// Fills the usable width; `width` is kept for going back.
    full: bool,
}

impl<W: Clone + PartialEq> Column<W> {
    fn new(w: W, width: f64) -> Self {
        Column { windows: vec![w], heights: vec![1.0], width, full: false }
    }

    fn fraction(&self) -> f64 {
        if self.full {
            1.0
        } else {
            self.width
        }
    }

    /// Adds `w` at `i`, taking an even share; the others shrink in step.
    fn insert(&mut self, i: usize, w: W) {
        let share = 1.0 / (self.windows.len() + 1) as f64;
        for h in &mut self.heights {
            *h *= 1.0 - share;
        }
        self.windows.insert(i, w);
        self.heights.insert(i, share);
    }

    /// Takes the window at `i` out; the others grow in step.
    fn remove(&mut self, i: usize) -> W {
        let w = self.windows.remove(i);
        self.heights.remove(i);
        let total: f64 = self.heights.iter().sum();
        if total > 0.0 {
            for h in &mut self.heights {
                *h /= total;
            }
        }
        w
    }
}

/// Where the columns are, for one usable area.
struct Geometry {
    /// The area inside the outer gaps.
    inner: Rect,
    gap: i32,
    /// Each column's left edge, from the strip's start.
    xs: Vec<i32>,
    /// Each column's width in pixels.
    widths: Vec<i32>,
}

impl Geometry {
    /// The strip's whole width.
    fn total(&self) -> i32 {
        match (self.xs.last(), self.widths.last()) {
            (Some(x), Some(w)) => x + w,
            _ => 0,
        }
    }

    /// The offset that puts column `c` in the middle.
    fn centred(&self, c: usize) -> i32 {
        self.xs[c] + self.widths[c] / 2 - self.inner.w / 2
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Scroll<W> {
    columns: Vec<Column<W>>,
    /// How far the strip is scrolled: the view's left edge, in pixels from
    /// the strip's start. [`Scroll::arrange`] clamps it.
    offset: i32,
    pub center: Center,
}

impl<W: Clone + PartialEq> Scroll<W> {
    pub fn new(center: Center) -> Self {
        Scroll { columns: Vec::new(), offset: 0, center }
    }

    /// Every window, column by column, each top to bottom.
    pub fn windows(&self) -> Vec<W> {
        self.columns.iter().flat_map(|c| c.windows.iter().cloned()).collect()
    }

    pub fn len(&self) -> usize {
        self.columns.iter().map(|c| c.windows.len()).sum()
    }

    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    pub fn contains(&self, w: &W) -> bool {
        self.find(w).is_some()
    }

    /// How many columns there are.
    pub fn columns(&self) -> usize {
        self.columns.len()
    }

    /// The column `w` is in and its place there.
    fn find(&self, w: &W) -> Option<(usize, usize)> {
        self.columns.iter().enumerate().find_map(|(c, col)| col.windows.iter().position(|x| x == w).map(|i| (c, i)))
    }

    /// Adds a window in a new column right of `focused`'s (at the end
    /// without one).
    pub fn insert(&mut self, w: W, focused: Option<&W>) {
        let at = focused.and_then(|f| self.find(f)).map_or(self.columns.len(), |(c, _)| c + 1);
        self.columns.insert(at, Column::new(w, DEFAULT_WIDTH));
    }

    /// Adds a window dropped on `target`'s `side`: a new column left or
    /// right of it, or into its column above or below it.
    pub fn insert_at(&mut self, w: W, target: &W, side: Dir) {
        let Some((c, i)) = self.find(target) else {
            self.insert(w, None);
            return;
        };
        match side {
            Dir::Left => self.columns.insert(c, Column::new(w, DEFAULT_WIDTH)),
            Dir::Right => self.columns.insert(c + 1, Column::new(w, DEFAULT_WIDTH)),
            Dir::Up => self.columns[c].insert(i, w),
            Dir::Down => self.columns[c].insert(i + 1, w),
        }
    }

    /// Takes a window out; an emptied column goes too.
    pub fn remove(&mut self, w: &W) -> bool {
        let Some((c, i)) = self.find(w) else { return false };
        self.columns[c].remove(i);
        if self.columns[c].windows.is_empty() {
            self.columns.remove(c);
        }
        true
    }

    /// Swaps two windows' places (each keeps the other's height share).
    /// Callers check both are here.
    pub fn swap(&mut self, a: &W, b: &W) {
        if let (Some((ca, ia)), Some((cb, ib))) = (self.find(a), self.find(b)) {
            let wa = std::mem::replace(&mut self.columns[ca].windows[ia], b.clone());
            self.columns[cb].windows[ib] = wa;
        }
    }

    /// Puts `new` in `old`'s place. Callers check `old` is here.
    pub fn replace(&mut self, old: &W, new: W) {
        if let Some((c, i)) = self.find(old) {
            self.columns[c].windows[i] = new;
        }
    }

    /// Moves `w` one step: left or right swaps its whole column with the
    /// next one (widths go with them); up or down swaps it within its
    /// column. False at the end of the strip or column.
    pub fn move_dir(&mut self, w: &W, dir: Dir) -> bool {
        let Some((c, i)) = self.find(w) else { return false };
        match dir {
            Dir::Left if c > 0 => self.columns.swap(c, c - 1),
            Dir::Right if c + 1 < self.columns.len() => self.columns.swap(c, c + 1),
            Dir::Up if i > 0 => self.columns[c].windows.swap(i, i - 1),
            Dir::Down if i + 1 < self.columns[c].windows.len() => self.columns[c].windows.swap(i, i + 1),
            _ => return false,
        }
        true
    }

    /// Pulls the top window of the column right of `w`'s into `w`'s column,
    /// just under `w` (SUPER+[). False if there's no column to the right.
    pub fn consume(&mut self, w: &W) -> bool {
        let Some((c, i)) = self.find(w) else { return false };
        if c + 1 >= self.columns.len() {
            return false;
        }
        let taken = self.columns[c + 1].remove(0);
        if self.columns[c + 1].windows.is_empty() {
            self.columns.remove(c + 1);
        }
        self.columns[c].insert(i + 1, taken);
        true
    }

    /// Pushes `w` out of its column into a new one to the right, as wide as
    /// the one it left (SUPER+]). False if it's alone in its column.
    pub fn expel(&mut self, w: &W) -> bool {
        let Some((c, i)) = self.find(w) else { return false };
        if self.columns[c].windows.len() < 2 {
            return false;
        }
        let width = self.columns[c].width;
        let w = self.columns[c].remove(i);
        self.columns.insert(c + 1, Column::new(w, width));
        true
    }

    /// Sets `w`'s column to the next preset width (SUPER+W): the first one
    /// wider than now, else the narrowest. A full-width column comes back.
    pub fn cycle_width(&mut self, w: &W) -> bool {
        let Some((c, _)) = self.find(w) else { return false };
        let col = &mut self.columns[c];
        col.width = WIDTHS.into_iter().find(|p| *p > col.width + 1e-3).unwrap_or(WIDTHS[0]);
        col.full = false;
        true
    }

    /// Makes `w`'s column fill the usable width, or gives it back its width
    /// (SUPER+SHIFT+W).
    pub fn toggle_full(&mut self, w: &W) -> bool {
        let Some((c, _)) = self.find(w) else { return false };
        self.columns[c].full = !self.columns[c].full;
        true
    }

    /// Whether `w`'s column fills the usable width.
    pub fn is_full(&self, w: &W) -> bool {
        self.find(w).is_some_and(|(c, _)| self.columns[c].full)
    }

    /// `w`'s column width as a fraction of the usable width.
    pub fn width_of(&self, w: &W) -> Option<f64> {
        self.find(w).map(|(c, _)| self.columns[c].fraction())
    }

    /// Moves `w`'s `edge` outwards by `delta` pixels (inwards if negative).
    /// Left and right change its column's width (any width, not only the
    /// presets); on the left edge the view moves with it, so the right edge
    /// stays put on screen. Up and down move the split with the window
    /// above or below. False for a column's top or bottom.
    pub fn resize_edge(&mut self, w: &W, edge: Dir, delta: i32, area: Rect, gaps: &Gaps) -> bool {
        let Some((c, i)) = self.find(w) else { return false };
        let g = self.geometry(area, gaps);
        match edge {
            Dir::Left | Dir::Right => {
                let unit = (g.inner.w + g.gap).max(1) as f64;
                let col = &mut self.columns[c];
                let before = g.widths[c];
                let want = (before + delta + g.gap) as f64 / unit;
                col.width = want.clamp(MIN_WIDTH, 1.0);
                col.full = false;
                if edge == Dir::Left {
                    let after = self.geometry(area, gaps).widths[c];
                    self.offset = self.clamped(&g) + (after - before);
                }
                true
            }
            Dir::Up | Dir::Down => {
                let j = match edge {
                    Dir::Up if i > 0 => i - 1,
                    Dir::Down if i + 1 < self.columns[c].windows.len() => i + 1,
                    _ => return false,
                };
                let col = &mut self.columns[c];
                let n = col.windows.len() as i32;
                let avail = (g.inner.h - g.gap * (n - 1)).max(1) as f64;
                let pair = col.heights[i] + col.heights[j];
                let min = MIN_HEIGHT.min(pair / 2.0);
                let mine = (col.heights[i] + delta as f64 / avail).clamp(min, pair - min);
                col.heights[i] = mine;
                col.heights[j] = pair - mine;
                true
            }
        }
    }

    /// Every window's rectangle within `area` scrolled by the view offset;
    /// windows off screen get rectangles outside it.
    pub fn arrange(&self, area: Rect, gaps: &Gaps) -> Vec<(W, Rect)> {
        let g = self.geometry(area, gaps);
        let offset = self.clamped(&g);
        let mut out = Vec::with_capacity(self.len());
        for (c, col) in self.columns.iter().enumerate() {
            let x = g.inner.x + g.xs[c] - offset;
            let n = col.windows.len() as i32;
            let avail = (g.inner.h - g.gap * (n - 1)).max(0);
            let mut share = 0.0;
            let mut top = 0;
            for (k, (w, h)) in col.windows.iter().zip(&col.heights).enumerate() {
                share += h;
                // The last window takes what's left, so rounding never
                // leaves a gap at the bottom.
                let bottom = if k + 1 == col.windows.len() { avail } else { (avail as f64 * share).round() as i32 };
                let y = g.inner.y + top + g.gap * k as i32;
                out.push((w.clone(), Rect::new(x, y, g.widths[c], (bottom - top).max(0))));
                top = bottom;
            }
        }
        out
    }

    /// Scrolls so `w`'s column is in view, following the centring setting.
    /// `previous` is the window focused before it (for `Overflow`). True if
    /// the view moved.
    pub fn ensure_visible(&mut self, w: &W, previous: Option<&W>, area: Rect, gaps: &Gaps) -> bool {
        let Some((c, _)) = self.find(w) else { return false };
        let g = self.geometry(area, gaps);
        let before = self.clamped(&g);
        let view = g.inner.w;
        let (x0, x1) = (g.xs[c], g.xs[c] + g.widths[c]);
        // Just enough to show it.
        let shown = if x0 < before {
            x0
        } else if x1 > before + view {
            x1 - view
        } else {
            before
        };
        let want = match self.center {
            Center::Always => g.centred(c),
            Center::Never => shown,
            Center::Overflow => {
                let in_view = x0 >= before && x1 <= before + view;
                let prev = previous.and_then(|p| self.find(p)).map(|(p, _)| p).filter(|p| *p != c);
                match prev {
                    _ if in_view => before,
                    Some(p) => {
                        let (lo, hi) = (p.min(c), p.max(c));
                        let span = g.xs[hi] + g.widths[hi] - g.xs[lo];
                        if span <= view {
                            shown
                        } else {
                            g.centred(c)
                        }
                    }
                    None => shown,
                }
            }
        };
        self.offset = self.clamp(&g, want);
        self.offset != before
    }

    /// Scrolls the view by `delta` pixels (positive: towards the right end).
    /// True if it moved; it stops at either end.
    pub fn scroll_by(&mut self, delta: i32, area: Rect, gaps: &Gaps) -> bool {
        let g = self.geometry(area, gaps);
        let before = self.clamped(&g);
        self.offset = self.clamp(&g, before.saturating_add(delta));
        self.offset != before
    }

    /// The view offset as [`Scroll::arrange`] uses it.
    pub fn offset(&self, area: Rect, gaps: &Gaps) -> i32 {
        self.clamped(&self.geometry(area, gaps))
    }

    fn geometry(&self, area: Rect, gaps: &Gaps) -> Geometry {
        let (inner, gap) = gaps.apply(area, self.len());
        let unit = (inner.w + gap) as f64;
        let mut xs = Vec::with_capacity(self.columns.len());
        let mut widths = Vec::with_capacity(self.columns.len());
        let mut x = 0;
        for col in &self.columns {
            // Fractions of (width + one gap), so two halves or three thirds
            // fill the area exactly with gaps between them.
            let w = ((unit * col.fraction()).round() as i32 - gap).clamp(1, inner.w.max(1));
            xs.push(x);
            widths.push(w);
            x += w + gap;
        }
        Geometry { inner, gap, xs, widths }
    }

    /// How far the view can scroll. Centring always lets the first and last
    /// columns reach the middle; otherwise the strip stays against the
    /// area's left edge and never scrolls past its right end.
    fn bounds(&self, g: &Geometry) -> (i32, i32) {
        if g.xs.is_empty() {
            return (0, 0);
        }
        match self.center {
            Center::Always => (g.centred(0), g.centred(g.xs.len() - 1)),
            Center::Never | Center::Overflow => (0, (g.total() - g.inner.w).max(0)),
        }
    }

    fn clamp(&self, g: &Geometry, offset: i32) -> i32 {
        let (lo, hi) = self.bounds(g);
        offset.clamp(lo, hi.max(lo))
    }

    fn clamped(&self, g: &Geometry) -> i32 {
        self.clamp(g, self.offset)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Rect = Rect::new(0, 0, 1200, 600);
    const NO_GAPS: Gaps = Gaps { inner: 0, outer: 0, smart: false };

    fn strip(n: u32, center: Center) -> Scroll<u32> {
        let mut s = Scroll::new(center);
        for w in 1..=n {
            let last = s.windows().last().copied();
            s.insert(w, last.as_ref());
        }
        s
    }

    fn rect_of(s: &Scroll<u32>, w: u32) -> Rect {
        s.arrange(AREA, &NO_GAPS).into_iter().find(|(x, _)| *x == w).unwrap().1
    }

    #[test]
    fn center_names_round_trip() {
        for c in Center::ALL {
            assert_eq!(Center::parse(c.as_str()), Some(c));
        }
        assert_eq!(Center::parse("sometimes"), None);
        assert_eq!(Center::default(), Center::Overflow);
    }

    #[test]
    fn new_columns_are_half_wide_and_run_off_the_right() {
        let s = strip(3, Center::Never);
        assert_eq!(
            s.arrange(AREA, &NO_GAPS),
            vec![(1, Rect::new(0, 0, 600, 600)), (2, Rect::new(600, 0, 600, 600)), (3, Rect::new(1200, 0, 600, 600))]
        );
        assert_eq!(s.columns(), 3);
        assert!(strip(0, Center::Never).arrange(AREA, &NO_GAPS).is_empty());
    }

    #[test]
    fn a_new_window_goes_right_of_the_focused_column() {
        let mut s = strip(3, Center::Never);
        s.insert(4, Some(&1));
        assert_eq!(s.windows(), vec![1, 4, 2, 3]);
        s.insert(5, None);
        assert_eq!(s.windows(), vec![1, 4, 2, 3, 5]);
    }

    #[test]
    fn gaps_fill_the_area_exactly_with_halves_and_thirds() {
        let g = Gaps { inner: 10, outer: 20, smart: false };
        let s = strip(2, Center::Never);
        let tiles = s.arrange(AREA, &g);
        assert_eq!(tiles[0].1, Rect::new(20, 20, 575, 560));
        assert_eq!(tiles[1].1, Rect::new(605, 20, 575, 560));
        assert_eq!(tiles[1].1.right(), 1180, "ends at the outer gap");

        let mut s = strip(3, Center::Never);
        for w in 1..=3 {
            s.cycle_width(&w); // 1/2 -> 2/3
            s.cycle_width(&w); // -> 1/3
        }
        let tiles = s.arrange(AREA, &g);
        assert_eq!(tiles[2].1.right(), 1180);
        assert_eq!(tiles[1].1.x - tiles[0].1.right(), 10);
    }

    #[test]
    fn smart_gaps_drop_for_a_lone_window() {
        let g = Gaps { inner: 10, outer: 20, smart: true };
        let mut s = strip(1, Center::Never);
        s.toggle_full(&1);
        assert_eq!(s.arrange(AREA, &g), vec![(1, AREA)]);
    }

    #[test]
    fn stacked_windows_share_the_column() {
        let mut s = strip(2, Center::Never);
        assert!(s.consume(&1));
        assert_eq!(s.columns(), 1);
        assert_eq!(s.arrange(AREA, &NO_GAPS), vec![(1, Rect::new(0, 0, 600, 300)), (2, Rect::new(0, 300, 600, 300))]);
        let g = Gaps { inner: 10, outer: 0, smart: false };
        let tiles = s.arrange(AREA, &g);
        assert_eq!(tiles[0].1, Rect::new(0, 0, 595, 295));
        assert_eq!(tiles[1].1, Rect::new(0, 305, 595, 295));
    }

    #[test]
    fn consume_takes_the_top_of_the_next_column_under_the_focused_window() {
        let mut s = strip(4, Center::Never);
        assert!(s.consume(&2)); // 3 joins 2
        assert!(!s.consume(&4), "nothing to the right");
        assert!(s.consume(&1)); // 2 (the top of the next column) joins 1
        assert_eq!(s.windows(), vec![1, 2, 3, 4]);
        assert_eq!(s.columns(), 3);
        assert!(s.consume(&1)); // 3 joins 1 under it
        assert_eq!(s.windows(), vec![1, 3, 2, 4]);
        assert_eq!(s.columns(), 2);
    }

    #[test]
    fn expel_makes_a_column_as_wide_as_the_old_one() {
        let mut s = strip(2, Center::Never);
        s.cycle_width(&1); // 2/3
        s.consume(&1);
        assert!(s.expel(&1));
        assert_eq!(s.windows(), vec![2, 1]);
        assert_eq!(s.width_of(&1), Some(2.0 / 3.0));
        assert!(!s.expel(&1), "alone in its column");
        assert_eq!(rect_of(&s, 2).h, 600, "the one left behind fills the column");
    }

    #[test]
    fn width_cycles_through_the_presets() {
        let mut s = strip(1, Center::Never);
        assert_eq!(s.width_of(&1), Some(0.5));
        s.cycle_width(&1);
        assert_eq!(s.width_of(&1), Some(2.0 / 3.0));
        s.cycle_width(&1);
        assert_eq!(s.width_of(&1), Some(1.0 / 3.0));
        s.cycle_width(&1);
        assert_eq!(s.width_of(&1), Some(0.5));
        assert!(!s.cycle_width(&9));
    }

    #[test]
    fn full_width_comes_back_to_the_old_width() {
        let mut s = strip(2, Center::Never);
        assert!(s.toggle_full(&2));
        assert!(s.is_full(&2));
        assert_eq!(rect_of(&s, 2).w, 1200);
        s.toggle_full(&2);
        assert_eq!(s.width_of(&2), Some(0.5));
        s.toggle_full(&2);
        s.cycle_width(&2);
        assert!(!s.is_full(&2), "cycling leaves full width");
        assert_eq!(s.width_of(&2), Some(2.0 / 3.0));
    }

    #[test]
    fn never_scrolls_just_enough() {
        let mut s = strip(4, Center::Never);
        assert_eq!(s.offset(AREA, &NO_GAPS), 0);
        assert!(s.ensure_visible(&3, None, AREA, &NO_GAPS));
        assert_eq!(s.offset(AREA, &NO_GAPS), 600, "column 3's right edge at the screen's");
        assert_eq!(rect_of(&s, 3), Rect::new(600, 0, 600, 600));
        assert!(!s.ensure_visible(&2, None, AREA, &NO_GAPS), "already in view");
        assert!(s.ensure_visible(&1, None, AREA, &NO_GAPS));
        assert_eq!(rect_of(&s, 1).x, 0);
    }

    #[test]
    fn overflow_centres_when_the_previous_column_wont_fit_beside_it() {
        let mut s = strip(4, Center::Overflow);
        // From 2 to 3: both fit (two halves), so it scrolls just enough.
        s.ensure_visible(&3, Some(&2), AREA, &NO_GAPS);
        assert_eq!(rect_of(&s, 3).x, 600);
        // From 1 (off screen now) to 3: 1..3 is 1800 wide, so 3 is centred.
        s.ensure_visible(&1, None, AREA, &NO_GAPS);
        s.ensure_visible(&3, Some(&1), AREA, &NO_GAPS);
        assert_eq!(rect_of(&s, 3).x, 300);
        // Already in view: nothing moves.
        assert!(!s.ensure_visible(&3, Some(&4), AREA, &NO_GAPS));
    }

    #[test]
    fn overflow_without_a_previous_column_scrolls_just_enough() {
        let mut s = strip(4, Center::Overflow);
        s.ensure_visible(&4, None, AREA, &NO_GAPS);
        assert_eq!(rect_of(&s, 4).right(), 1200);
    }

    #[test]
    fn always_centres_even_the_ends() {
        let mut s = strip(3, Center::Always);
        s.ensure_visible(&1, None, AREA, &NO_GAPS);
        assert_eq!(rect_of(&s, 1).x, 300, "empty space on its left");
        s.ensure_visible(&3, None, AREA, &NO_GAPS);
        assert_eq!(rect_of(&s, 3).x, 300);
    }

    #[test]
    fn scrolling_stops_at_the_ends() {
        let mut s = strip(3, Center::Never);
        assert!(!s.scroll_by(-100, AREA, &NO_GAPS), "already at the start");
        assert!(s.scroll_by(250, AREA, &NO_GAPS));
        assert_eq!(rect_of(&s, 1).x, -250);
        assert!(s.scroll_by(10_000, AREA, &NO_GAPS));
        assert_eq!(rect_of(&s, 3).right(), 1200, "the last column ends at the screen's edge");
        assert!(!s.scroll_by(1, AREA, &NO_GAPS));
        // A strip narrower than the screen doesn't scroll at all.
        let mut s = strip(2, Center::Never);
        assert!(!s.scroll_by(100, AREA, &NO_GAPS));
    }

    #[test]
    fn a_narrower_area_rescales_and_reclamps() {
        let mut s = strip(3, Center::Never);
        s.scroll_by(10_000, AREA, &NO_GAPS);
        let small = Rect::new(0, 0, 800, 600);
        let tiles = s.arrange(small, &NO_GAPS);
        assert_eq!(tiles[0].1.w, 400, "still half wide");
        assert_eq!(tiles[2].1.right(), 800, "the old offset is clamped to the new end");
    }

    #[test]
    fn removing_the_last_window_of_a_column_closes_it() {
        let mut s = strip(3, Center::Never);
        s.consume(&1);
        assert!(s.remove(&2));
        assert_eq!(s.columns(), 2);
        assert_eq!(rect_of(&s, 1).h, 600);
        assert!(s.remove(&1));
        assert_eq!(s.columns(), 1);
        assert!(!s.remove(&1));
        assert_eq!(rect_of(&s, 3).x, 0);
    }

    #[test]
    fn insert_at_makes_a_column_or_joins_one() {
        let mut s = strip(2, Center::Never);
        s.insert_at(3, &2, Dir::Left);
        s.insert_at(4, &2, Dir::Right);
        assert_eq!(s.windows(), vec![1, 3, 2, 4]);
        assert_eq!(s.columns(), 4);
        s.insert_at(5, &2, Dir::Up);
        s.insert_at(6, &2, Dir::Down);
        assert_eq!(s.windows(), vec![1, 3, 5, 2, 6, 4]);
        assert_eq!(s.columns(), 4);
        assert_eq!(rect_of(&s, 2).h, 200);
        s.insert_at(7, &99, Dir::Left);
        assert_eq!(s.windows().last(), Some(&7), "unknown target: at the end");
    }

    #[test]
    fn moving_left_and_right_swaps_whole_columns() {
        let mut s = strip(3, Center::Never);
        s.cycle_width(&1); // 2/3
        s.consume(&2); // 3 under 2
        assert!(s.move_dir(&3, Dir::Left));
        assert_eq!(s.windows(), vec![2, 3, 1]);
        assert_eq!(s.width_of(&1), Some(2.0 / 3.0), "the width moves with its column");
        assert!(!s.move_dir(&2, Dir::Left), "first column");
        assert!(s.move_dir(&2, Dir::Down));
        assert_eq!(s.windows(), vec![3, 2, 1]);
        assert!(!s.move_dir(&2, Dir::Down), "bottom of the column");
        assert!(!s.move_dir(&1, Dir::Right), "last column");
    }

    #[test]
    fn swap_and_replace_keep_the_shape() {
        let mut s = strip(3, Center::Never);
        s.consume(&1);
        s.swap(&1, &3);
        assert_eq!(s.windows(), vec![3, 2, 1]);
        assert_eq!(rect_of(&s, 3), Rect::new(0, 0, 600, 300));
        s.replace(&2, 9);
        assert_eq!(s.windows(), vec![3, 9, 1]);
        assert!(!s.contains(&2));
    }

    #[test]
    fn dragging_a_side_edge_sets_any_width() {
        let mut s = strip(2, Center::Never);
        assert!(s.resize_edge(&1, Dir::Right, 120, AREA, &NO_GAPS));
        assert_eq!(rect_of(&s, 1).w, 720);
        assert_eq!(rect_of(&s, 2).x, 720, "the next column follows");
        assert!(s.resize_edge(&1, Dir::Right, -10_000, AREA, &NO_GAPS));
        assert_eq!(rect_of(&s, 1).w, 120, "clamped to the narrowest");
        assert!(s.resize_edge(&1, Dir::Right, 10_000, AREA, &NO_GAPS));
        assert_eq!(rect_of(&s, 1).w, 1200, "and the widest");
        assert!(!s.resize_edge(&9, Dir::Right, 10, AREA, &NO_GAPS));
    }

    #[test]
    fn dragging_a_left_edge_keeps_the_right_edge_still() {
        let mut s = strip(3, Center::Never);
        s.ensure_visible(&3, None, AREA, &NO_GAPS);
        let before = rect_of(&s, 2);
        assert!(s.resize_edge(&2, Dir::Left, 100, AREA, &NO_GAPS));
        let after = rect_of(&s, 2);
        assert_eq!((after.w, after.right()), (700, before.right()));
    }

    #[test]
    fn dragging_a_split_in_a_column_moves_it() {
        let mut s = strip(3, Center::Never);
        s.consume(&1);
        s.consume(&1); // 1, 3, 2 stacked
        assert!(s.resize_edge(&1, Dir::Down, 100, AREA, &NO_GAPS));
        let tiles = s.arrange(AREA, &NO_GAPS);
        assert_eq!(tiles.iter().map(|(_, r)| r.h).collect::<Vec<_>>(), vec![300, 100, 200]);
        assert!(s.resize_edge(&2, Dir::Up, 10_000, AREA, &NO_GAPS));
        let h = s.arrange(AREA, &NO_GAPS)[1].1.h;
        assert!(h > 0 && h < 100, "the window above keeps a sliver: {h}");
        assert!(!s.resize_edge(&1, Dir::Up, 10, AREA, &NO_GAPS), "top of the column");
        assert!(!s.resize_edge(&2, Dir::Down, 10, AREA, &NO_GAPS), "bottom of the column");
    }

    #[test]
    fn heights_survive_a_window_leaving() {
        let mut s = strip(3, Center::Never);
        s.consume(&1);
        s.consume(&1);
        s.resize_edge(&1, Dir::Down, 100, AREA, &NO_GAPS); // 300, 100, 200
        s.remove(&3);
        let tiles = s.arrange(AREA, &NO_GAPS);
        assert_eq!(tiles.iter().map(|(_, r)| r.h).collect::<Vec<_>>(), vec![360, 240], "3:2 kept");
    }
}
