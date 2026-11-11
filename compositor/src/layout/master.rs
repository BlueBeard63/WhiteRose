//! Master: the first window gets a large tile on one side; the others share
//! the rest of the area in a stack.

use super::{clamp_ratio, Axis, Dir, Gaps, Rect};

/// Which side of the area the master tile is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Side {
    #[default]
    Left,
    Right,
    Top,
    Bottom,
}

impl Side {
    pub const ALL: [Side; 4] = [Side::Left, Side::Right, Side::Top, Side::Bottom];

    /// The name used in settings.
    pub fn as_str(self) -> &'static str {
        match self {
            Side::Left => "left",
            Side::Right => "right",
            Side::Top => "top",
            Side::Bottom => "bottom",
        }
    }

    pub fn parse(s: &str) -> Option<Side> {
        Side::ALL.into_iter().find(|x| x.as_str() == s)
    }

    /// The axis the master and the stack are split along.
    fn axis(self) -> Axis {
        match self {
            Side::Left | Side::Right => Axis::Horizontal,
            Side::Top | Side::Bottom => Axis::Vertical,
        }
    }

    /// From the master tile towards the stack.
    fn towards_stack(self) -> Dir {
        match self {
            Side::Left => Dir::Right,
            Side::Right => Dir::Left,
            Side::Top => Dir::Down,
            Side::Bottom => Dir::Up,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Master<W> {
    /// The master first, then the stack from top (or left) down.
    windows: Vec<W>,
    /// The master tile's share of the area.
    pub ratio: f64,
    pub side: Side,
}

impl<W: Clone + PartialEq> Master<W> {
    pub fn new(ratio: f64, side: Side) -> Self {
        Master { windows: Vec::new(), ratio: clamp_ratio(ratio), side }
    }

    pub fn windows(&self) -> &[W] {
        &self.windows
    }

    /// Adds a window at the end of the stack (the master if it's the first).
    pub fn push(&mut self, w: W) {
        self.windows.push(w);
    }

    /// Adds a window just after `focused`, or at the end of the stack.
    pub fn insert(&mut self, w: W, focused: Option<&W>) {
        match focused.and_then(|f| self.index(f)) {
            Some(i) => self.windows.insert(i + 1, w),
            None => self.windows.push(w),
        }
    }

    /// Adds a window before `target` (dropped on its left or top) or after
    /// it (right or bottom).
    pub fn insert_at(&mut self, w: W, target: &W, side: Dir) {
        match self.index(target) {
            Some(i) => self.windows.insert(if side.forward() { i + 1 } else { i }, w),
            None => self.windows.push(w),
        }
    }

    pub fn remove(&mut self, w: &W) -> bool {
        let before = self.windows.len();
        self.windows.retain(|x| x != w);
        self.windows.len() != before
    }

    /// Swaps two tiled windows' places. Callers check both are tiled.
    pub fn swap(&mut self, a: &W, b: &W) {
        if let (Some(i), Some(j)) = (self.index(a), self.index(b)) {
            self.windows.swap(i, j);
        }
    }

    /// Makes `w` the master, or, if it already is, swaps it with the top of
    /// the stack.
    pub fn make_master(&mut self, w: &W) -> bool {
        match self.index(w) {
            Some(0) if self.windows.len() > 1 => self.windows.swap(0, 1),
            Some(i) if i > 0 => self.windows.swap(0, i),
            _ => return false,
        }
        true
    }

    pub fn arrange(&self, area: Rect, gaps: &Gaps) -> Vec<(W, Rect)> {
        let n = self.windows.len();
        let (area, gap) = gaps.apply(area, n);
        let rects = match n {
            0 => Vec::new(),
            1 => vec![area],
            _ => {
                let (master, stack) = self.split(area, gap);
                std::iter::once(master).chain(stack.divide(self.side.axis().other(), n - 1, gap)).collect()
            }
        };
        self.windows.iter().cloned().zip(rects).collect()
    }

    /// The master's tile and the stack's area.
    fn split(&self, area: Rect, gap: i32) -> (Rect, Rect) {
        match self.side {
            Side::Left | Side::Top => area.split(self.side.axis(), self.ratio, gap),
            Side::Right | Side::Bottom => {
                let (stack, master) = area.split(self.side.axis(), 1.0 - self.ratio, gap);
                (master, stack)
            }
        }
    }

    /// Moves `w`'s `edge` by `delta` pixels outwards. Only the line between
    /// the master and the stack moves; stack tiles share their space evenly.
    pub fn resize_edge(&mut self, w: &W, edge: Dir, delta: i32, area: Rect, gaps: &Gaps) -> bool {
        let n = self.windows.len();
        let Some(i) = self.index(w) else { return false };
        if n < 2 {
            return false;
        }
        let towards = self.side.towards_stack();
        let grow_master = match i {
            0 if edge == towards => 1.0,
            1.. if edge == towards.opposite() => -1.0,
            _ => return false,
        };
        let (area, gap) = gaps.apply(area, n);
        let extent = (area.extent(self.side.axis()) - gap).max(1) as f64;
        self.ratio = clamp_ratio(self.ratio + grow_master * delta as f64 / extent);
        true
    }

    fn index(&self, w: &W) -> Option<usize> {
        self.windows.iter().position(|x| x == w)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Rect = Rect::new(0, 0, 1000, 600);
    const NO_GAPS: Gaps = Gaps { inner: 0, outer: 0, smart: false };

    fn master(n: u32, side: Side) -> Master<u32> {
        let mut m = Master::new(0.6, side);
        for w in 1..=n {
            m.push(w);
        }
        m
    }

    #[test]
    fn side_names_round_trip() {
        for s in Side::ALL {
            assert_eq!(Side::parse(s.as_str()), Some(s));
        }
        assert_eq!(Side::parse("middle"), None);
    }

    #[test]
    fn a_lone_window_fills_the_area() {
        assert_eq!(master(1, Side::Left).arrange(AREA, &NO_GAPS), vec![(1, AREA)]);
        assert!(master(0, Side::Left).arrange(AREA, &NO_GAPS).is_empty());
    }

    #[test]
    fn master_on_the_left_with_a_stack() {
        assert_eq!(
            master(3, Side::Left).arrange(AREA, &NO_GAPS),
            vec![(1, Rect::new(0, 0, 600, 600)), (2, Rect::new(600, 0, 400, 300)), (3, Rect::new(600, 300, 400, 300))]
        );
    }

    #[test]
    fn every_side_works() {
        let tiles = master(3, Side::Right).arrange(AREA, &NO_GAPS);
        assert_eq!(tiles[0], (1, Rect::new(400, 0, 600, 600)));
        assert_eq!(tiles[1], (2, Rect::new(0, 0, 400, 300)));
        let tiles = master(3, Side::Top).arrange(AREA, &NO_GAPS);
        assert_eq!(tiles[0], (1, Rect::new(0, 0, 1000, 360)));
        assert_eq!(tiles[2], (3, Rect::new(500, 360, 500, 240)));
        let tiles = master(2, Side::Bottom).arrange(AREA, &NO_GAPS);
        assert_eq!(tiles, vec![(1, Rect::new(0, 240, 1000, 360)), (2, Rect::new(0, 0, 1000, 240))]);
    }

    #[test]
    fn gaps_separate_master_and_stack() {
        let g = Gaps { inner: 10, outer: 10, smart: false };
        let tiles = master(3, Side::Left).arrange(AREA, &g);
        assert_eq!(tiles[0], (1, Rect::new(10, 10, 582, 580)));
        assert_eq!(tiles[1], (2, Rect::new(602, 10, 388, 285)));
        assert_eq!(tiles[2], (3, Rect::new(602, 305, 388, 285)));
    }

    #[test]
    fn insert_goes_after_the_focused_window() {
        let mut m = master(3, Side::Left);
        m.insert(4, Some(&1));
        assert_eq!(m.windows(), &[1, 4, 2, 3]);
        m.insert(5, None);
        assert_eq!(m.windows(), &[1, 4, 2, 3, 5]);
        m.insert_at(6, &2, Dir::Up);
        m.insert_at(7, &2, Dir::Down);
        assert_eq!(m.windows(), &[1, 4, 6, 2, 7, 3, 5]);
    }

    #[test]
    fn removing_the_master_promotes_the_top_of_the_stack() {
        let mut m = master(3, Side::Left);
        assert!(m.remove(&1));
        assert_eq!(m.arrange(AREA, &NO_GAPS)[0], (2, Rect::new(0, 0, 600, 600)));
        assert!(!m.remove(&1));
    }

    #[test]
    fn make_master_swaps_with_the_master() {
        let mut m = master(3, Side::Left);
        assert!(m.make_master(&3));
        assert_eq!(m.windows(), &[3, 2, 1]);
        assert!(m.make_master(&3), "already master: swaps with the stack's top");
        assert_eq!(m.windows(), &[2, 3, 1]);
        assert!(!m.make_master(&9));
        assert!(!master(1, Side::Left).make_master(&1));
    }

    #[test]
    fn resize_moves_the_master_line() {
        let mut m = master(3, Side::Left);
        assert!(m.resize_edge(&1, Dir::Right, 100, AREA, &NO_GAPS));
        assert_eq!(m.arrange(AREA, &NO_GAPS)[0].1.w, 700);
        // The stack's left edge is the same line.
        assert!(m.resize_edge(&2, Dir::Left, 200, AREA, &NO_GAPS));
        assert_eq!(m.arrange(AREA, &NO_GAPS)[0].1.w, 500);
        assert!(!m.resize_edge(&1, Dir::Left, 100, AREA, &NO_GAPS), "the area's border");
        assert!(!m.resize_edge(&2, Dir::Down, 100, AREA, &NO_GAPS), "stack tiles share evenly");
        assert!(!master(1, Side::Left).resize_edge(&1, Dir::Right, 100, AREA, &NO_GAPS));
    }

    #[test]
    fn resize_on_the_right_side_runs_the_other_way() {
        let mut m = master(2, Side::Right);
        assert!(m.resize_edge(&1, Dir::Left, 100, AREA, &NO_GAPS));
        assert_eq!(m.arrange(AREA, &NO_GAPS)[0], (1, Rect::new(300, 0, 700, 600)));
        assert!(m.resize_edge(&1, Dir::Left, 10_000, AREA, &NO_GAPS));
        assert_eq!(m.arrange(AREA, &NO_GAPS)[0].1.w, 900, "clamped");
    }
}
