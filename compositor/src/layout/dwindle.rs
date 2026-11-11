//! Dwindle: a binary tree of splits. Each new window splits a tile in two,
//! so the tiles spiral inwards as windows open.

use super::{clamp_ratio, Axis, Dir, Gaps, Rect};

#[derive(Debug, Clone, PartialEq)]
enum Node<W> {
    Leaf(W),
    Split {
        /// None: follow the shape of the split's rectangle (side by side
        /// when wide, stacked when tall), so a tree adapts to any output.
        axis: Option<Axis>,
        /// The first child's share.
        ratio: f64,
        first: Box<Node<W>>,
        second: Box<Node<W>>,
    },
}

impl<W: Clone + PartialEq> Node<W> {
    fn contains(&self, w: &W) -> bool {
        match self {
            Node::Leaf(x) => x == w,
            Node::Split { first, second, .. } => first.contains(w) || second.contains(w),
        }
    }

    fn collect(&self, out: &mut Vec<W>) {
        match self {
            Node::Leaf(x) => out.push(x.clone()),
            Node::Split { first, second, .. } => {
                first.collect(out);
                second.collect(out);
            }
        }
    }

    /// The leaf holding `w`.
    fn leaf_mut(&mut self, w: &W) -> Option<&mut Node<W>> {
        if matches!(self, Node::Leaf(x) if x == w) {
            return Some(self);
        }
        match self {
            Node::Split { first, second, .. } if first.contains(w) => first.leaf_mut(w),
            Node::Split { second, .. } => second.leaf_mut(w),
            Node::Leaf(_) => None,
        }
    }

    /// The tree without `w`: its sibling takes its parent split's place.
    fn without(self, w: &W) -> Option<Node<W>> {
        match self {
            Node::Leaf(x) if &x == w => None,
            Node::Leaf(x) => Some(Node::Leaf(x)),
            Node::Split { axis, ratio, first, second } => match (first.without(w), second.without(w)) {
                (Some(a), Some(b)) => Some(Node::Split { axis, ratio, first: Box::new(a), second: Box::new(b) }),
                (Some(n), None) | (None, Some(n)) => Some(n),
                (None, None) => None,
            },
        }
    }

    fn for_each_leaf(&mut self, f: &mut impl FnMut(&mut W)) {
        match self {
            Node::Leaf(x) => f(x),
            Node::Split { first, second, .. } => {
                first.for_each_leaf(f);
                second.for_each_leaf(f);
            }
        }
    }

    fn arrange(&self, rect: Rect, gap: i32, out: &mut Vec<(W, Rect)>) {
        match self {
            Node::Leaf(x) => out.push((x.clone(), rect)),
            Node::Split { axis, ratio, first, second } => {
                let (a, b) = rect.split(axis.unwrap_or(rect.long_axis()), *ratio, gap);
                first.arrange(a, gap, out);
                second.arrange(b, gap, out);
            }
        }
    }

    /// Walks down to `w`'s leaf, then back up through its ancestors, nearest
    /// first, calling `f` on each split (with its rectangle, the axis it
    /// splits along and whether `w` is in its first child) until `f` returns
    /// true. None if `w` isn't in this subtree, else whether `f` did.
    fn up_from(
        &mut self,
        w: &W,
        rect: Rect,
        gap: i32,
        f: &mut impl FnMut(&mut Option<Axis>, &mut f64, Rect, Axis, bool) -> bool,
    ) -> Option<bool> {
        match self {
            Node::Leaf(x) => (x == w).then_some(false),
            Node::Split { axis, ratio, first, second } => {
                let along = axis.unwrap_or(rect.long_axis());
                let (a, b) = rect.split(along, *ratio, gap);
                let (done, in_first) = match first.up_from(w, a, gap, f) {
                    Some(done) => (done, true),
                    None => (second.up_from(w, b, gap, f)?, false),
                };
                Some(done || f(axis, ratio, rect, along, in_first))
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Dwindle<W> {
    root: Option<Node<W>>,
    /// The axis for new splits; None picks it from each tile's shape.
    pub split_rule: Option<Axis>,
}

impl<W: Clone + PartialEq> Dwindle<W> {
    pub fn new(split_rule: Option<Axis>) -> Self {
        Dwindle { root: None, split_rule }
    }

    pub fn windows(&self) -> Vec<W> {
        let mut out = Vec::new();
        if let Some(root) = &self.root {
            root.collect(&mut out);
        }
        out
    }

    pub fn len(&self) -> usize {
        self.windows().len()
    }

    pub fn is_empty(&self) -> bool {
        self.root.is_none()
    }

    pub fn contains(&self, w: &W) -> bool {
        self.root.as_ref().is_some_and(|r| r.contains(w))
    }

    /// The last tile in tree order: the innermost one of the spiral.
    pub fn last(&self) -> Option<&W> {
        let mut node = self.root.as_ref()?;
        loop {
            match node {
                Node::Leaf(x) => return Some(x),
                Node::Split { second, .. } => node = second,
            }
        }
    }

    /// Splits `target`'s tile (or the last tile, if `target` isn't tiled
    /// here), the new window taking the second half.
    pub fn insert(&mut self, w: W, target: Option<&W>) {
        let target = target.filter(|t| self.contains(t)).or(self.last()).cloned();
        match target {
            Some(t) => self.split_leaf(&t, w, self.split_rule, false),
            None => self.root = Some(Node::Leaf(w)),
        }
    }

    /// Splits `target`'s tile, the new window taking its `side`.
    pub fn insert_at(&mut self, w: W, target: &W, side: Dir) {
        if self.contains(target) {
            self.split_leaf(target, w, Some(side.axis()), !side.forward());
        } else {
            self.insert(w, None);
        }
    }

    fn split_leaf(&mut self, target: &W, w: W, axis: Option<Axis>, new_first: bool) {
        let Some(leaf) = self.root.as_mut().and_then(|r| r.leaf_mut(target)) else { return };
        let old = std::mem::replace(leaf, Node::Leaf(w.clone()));
        let (first, second) = if new_first { (Node::Leaf(w), old) } else { (old, Node::Leaf(w)) };
        *leaf = Node::Split { axis, ratio: 0.5, first: Box::new(first), second: Box::new(second) };
    }

    pub fn remove(&mut self, w: &W) -> bool {
        if !self.contains(w) {
            return false;
        }
        self.root = self.root.take().and_then(|r| r.without(w));
        true
    }

    /// Swaps two tiled windows' places. Callers check both are tiled.
    pub fn swap(&mut self, a: &W, b: &W) {
        if let Some(root) = &mut self.root {
            root.for_each_leaf(&mut |x| {
                if x == a {
                    *x = b.clone();
                } else if x == b {
                    *x = a.clone();
                }
            });
        }
    }

    pub fn arrange(&self, area: Rect, gaps: &Gaps) -> Vec<(W, Rect)> {
        let mut out = Vec::new();
        if let Some(root) = &self.root {
            let (area, gap) = gaps.apply(area, self.len());
            root.arrange(area, gap, &mut out);
        }
        out
    }

    /// Moves `w`'s `edge` by `delta` pixels outwards: the nearest split
    /// whose dividing line is that edge takes the change.
    pub fn resize_edge(&mut self, w: &W, edge: Dir, delta: i32, area: Rect, gaps: &Gaps) -> bool {
        let (area, gap) = gaps.apply(area, self.len());
        let Some(root) = &mut self.root else { return false };
        let found = root.up_from(w, area, gap, &mut |_, ratio, rect, along, in_first| {
            // The line sits after the first child and before the second.
            if along != edge.axis() || in_first != edge.forward() {
                return false;
            }
            let extent = (rect.extent(along) - gap).max(1) as f64;
            let change = delta as f64 / extent;
            *ratio = clamp_ratio(if in_first { *ratio + change } else { *ratio - change });
            true
        });
        found == Some(true)
    }

    /// Flips the split `w`'s tile is in (side by side ⇄ stacked).
    pub fn toggle_split(&mut self, w: &W, area: Rect, gaps: &Gaps) -> bool {
        let (area, gap) = gaps.apply(area, self.len());
        let Some(root) = &mut self.root else { return false };
        let found = root.up_from(w, area, gap, &mut |axis, _, _, along, _| {
            *axis = Some(along.other());
            true
        });
        found == Some(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AREA: Rect = Rect::new(0, 0, 1000, 600);
    const NO_GAPS: Gaps = Gaps { inner: 0, outer: 0, smart: false };

    fn spiral(n: u32) -> Dwindle<u32> {
        let mut d = Dwindle::new(None);
        for w in 1..=n {
            let last = d.last().cloned();
            d.insert(w, last.as_ref());
        }
        d
    }

    fn rect_of(d: &Dwindle<u32>, w: u32) -> Rect {
        d.arrange(AREA, &NO_GAPS).into_iter().find(|(x, _)| *x == w).unwrap().1
    }

    #[test]
    fn windows_spiral_inwards() {
        let d = spiral(4);
        assert_eq!(
            d.arrange(AREA, &NO_GAPS),
            vec![
                (1, Rect::new(0, 0, 500, 600)),
                (2, Rect::new(500, 0, 500, 300)),
                (3, Rect::new(500, 300, 250, 300)),
                (4, Rect::new(750, 300, 250, 300)),
            ]
        );
    }

    #[test]
    fn one_window_fills_the_area_within_the_outer_gap() {
        let d = spiral(1);
        let g = Gaps { inner: 10, outer: 20, smart: false };
        assert_eq!(d.arrange(AREA, &g), vec![(1, Rect::new(20, 20, 960, 560))]);
        assert!(Dwindle::<u32>::new(None).arrange(AREA, &g).is_empty());
    }

    #[test]
    fn gaps_separate_tiles() {
        let d = spiral(2);
        let g = Gaps { inner: 10, outer: 20, smart: true };
        assert_eq!(d.arrange(AREA, &g), vec![(1, Rect::new(20, 20, 475, 560)), (2, Rect::new(505, 20, 475, 560))]);
    }

    #[test]
    fn auto_splits_follow_the_area_shape() {
        // The same tree on a portrait monitor stacks instead.
        let d = spiral(2);
        let tall = Rect::new(0, 0, 600, 1000);
        assert_eq!(d.arrange(tall, &NO_GAPS)[1], (2, Rect::new(0, 500, 600, 500)));
    }

    #[test]
    fn a_split_rule_forces_the_axis() {
        let mut d = Dwindle::new(Some(Axis::Vertical));
        d.insert(1, None);
        d.insert(2, Some(&1));
        assert_eq!(d.arrange(AREA, &NO_GAPS)[1], (2, Rect::new(0, 300, 1000, 300)));
    }

    #[test]
    fn insert_splits_the_target_tile() {
        let mut d = spiral(2);
        d.insert(3, Some(&1));
        // 1's half is tall, so it's stacked: 1 above, 3 below; 2 untouched.
        assert_eq!(rect_of(&d, 1), Rect::new(0, 0, 500, 300));
        assert_eq!(rect_of(&d, 3), Rect::new(0, 300, 500, 300));
        assert_eq!(rect_of(&d, 2), Rect::new(500, 0, 500, 600));
        // An unknown target falls back to the last tile.
        d.insert(4, Some(&99));
        assert_eq!(d.windows(), vec![1, 3, 2, 4]);
    }

    #[test]
    fn insert_at_puts_the_window_on_that_side() {
        let mut d = spiral(1);
        d.insert_at(2, &1, Dir::Left);
        assert_eq!(rect_of(&d, 2), Rect::new(0, 0, 500, 600));
        d.insert_at(3, &1, Dir::Down);
        assert_eq!(rect_of(&d, 1), Rect::new(500, 0, 500, 300));
        assert_eq!(rect_of(&d, 3), Rect::new(500, 300, 500, 300));
        d.insert_at(4, &99, Dir::Up);
        assert!(d.contains(&4), "unknown target: inserted anyway");
    }

    #[test]
    fn removing_lets_the_sibling_take_the_space() {
        let mut d = spiral(3);
        assert!(d.remove(&2));
        assert_eq!(rect_of(&d, 3), Rect::new(500, 0, 500, 600));
        assert!(d.remove(&1));
        assert_eq!(d.arrange(AREA, &NO_GAPS), vec![(3, AREA)]);
        assert!(!d.remove(&1));
        assert!(d.remove(&3));
        assert!(d.is_empty());
        assert_eq!(d.last(), None);
    }

    #[test]
    fn swap_exchanges_places() {
        let mut d = spiral(3);
        d.swap(&1, &3);
        assert_eq!(d.windows(), vec![3, 2, 1]);
        assert_eq!(rect_of(&d, 3), Rect::new(0, 0, 500, 600));
    }

    #[test]
    fn resize_moves_the_split_on_that_edge() {
        let mut d = spiral(3);
        // 1's right edge is the root split.
        assert!(d.resize_edge(&1, Dir::Right, 100, AREA, &NO_GAPS));
        assert_eq!(rect_of(&d, 1).w, 600);
        assert_eq!(rect_of(&d, 2), Rect::new(600, 0, 400, 300));
        // 3's left edge is that same split, seen from the other side.
        assert!(d.resize_edge(&3, Dir::Left, 100, AREA, &NO_GAPS));
        assert_eq!(rect_of(&d, 1).w, 500);
        // 3's top edge is the split between 2 and 3: pulling it in shrinks 3.
        assert!(d.resize_edge(&3, Dir::Up, -100, AREA, &NO_GAPS));
        assert_eq!(rect_of(&d, 2).h, 400);
        assert_eq!(rect_of(&d, 3), Rect::new(500, 400, 500, 200));
        // The area's border can't move.
        assert!(!d.resize_edge(&1, Dir::Left, 50, AREA, &NO_GAPS));
        assert!(!d.resize_edge(&2, Dir::Up, 50, AREA, &NO_GAPS));
        assert!(!d.resize_edge(&99, Dir::Up, 50, AREA, &NO_GAPS));
    }

    #[test]
    fn resize_stops_at_the_ratio_limits() {
        let mut d = spiral(2);
        assert!(d.resize_edge(&1, Dir::Right, 10_000, AREA, &NO_GAPS));
        assert_eq!(rect_of(&d, 1).w, 900);
        assert!(d.resize_edge(&1, Dir::Right, -10_000, AREA, &NO_GAPS));
        assert_eq!(rect_of(&d, 1).w, 100);
    }

    #[test]
    fn toggle_split_flips_the_parent_split() {
        let mut d = spiral(2);
        assert!(d.toggle_split(&2, AREA, &NO_GAPS));
        assert_eq!(rect_of(&d, 1), Rect::new(0, 0, 1000, 300));
        assert_eq!(rect_of(&d, 2), Rect::new(0, 300, 1000, 300));
        assert!(d.toggle_split(&1, AREA, &NO_GAPS));
        assert_eq!(rect_of(&d, 1), Rect::new(0, 0, 500, 600));
        let mut lone = spiral(1);
        assert!(!lone.toggle_split(&1, AREA, &NO_GAPS));
    }
}
