//! SUPER+CTRL+arrow: the focused window to the monitor on that side.
//! Tiled, it joins that monitor's tiles (out of its group first); floating
//! or snapped, it keeps its place relative to the screen; maximized or
//! fullscreen, it fills the other screen. It stays focused. Scratchpads
//! have their own rules and stay.

use smithay::{
    desktop::Window,
    output::Output,
    reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State as ToplevelState,
    utils::{Logical, Point, Rectangle},
};

use crate::{layout::Dir, state::WhiteRose};

/// The rectangle (of `others`) on the `dir` side of `from`: one past its
/// edge that way, the one lined up with it most (sharing the most of the
/// other axis), then the nearest. Its index, if there's one.
pub fn toward(from: Rectangle<i32, Logical>, others: &[Rectangle<i32, Logical>], dir: Dir) -> Option<usize> {
    let (fx0, fy0, fx1, fy1) = (from.loc.x, from.loc.y, from.loc.x + from.size.w, from.loc.y + from.size.h);
    others
        .iter()
        .enumerate()
        .filter(|(_, o)| **o != from)
        .filter_map(|(i, o)| {
            let (ox0, oy0, ox1, oy1) = (o.loc.x, o.loc.y, o.loc.x + o.size.w, o.loc.y + o.size.h);
            // How far past the edge it starts, and how much of the other
            // axis the two share.
            let (gap, shared) = match dir {
                Dir::Right if ox0 >= fx1 - 1 => (ox0 - fx1, fy1.min(oy1) - fy0.max(oy0)),
                Dir::Left if ox1 <= fx0 + 1 => (fx0 - ox1, fy1.min(oy1) - fy0.max(oy0)),
                Dir::Down if oy0 >= fy1 - 1 => (oy0 - fy1, fx1.min(ox1) - fx0.max(ox0)),
                Dir::Up if oy1 <= fy0 + 1 => (fy0 - oy1, fx1.min(ox1) - fx0.max(ox0)),
                _ => return None,
            };
            Some((i, gap.max(0), shared))
        })
        // Lined up first (sharing some of the other axis), then the nearest,
        // then the one sharing the most.
        .min_by_key(|&(_, gap, shared)| (shared <= 0, gap, -shared))
        .map(|(i, _, _)| i)
}

/// Where a window at `loc` (`size` big) in `from` goes in `to`: at the same
/// place relative to the screen, kept inside it.
pub fn carried(
    loc: Point<i32, Logical>,
    size: (i32, i32),
    from: Rectangle<i32, Logical>,
    to: Rectangle<i32, Logical>,
) -> Point<i32, Logical> {
    let rel = |p: i32, a: i32, len: i32| (p - a) as f64 / len.max(1) as f64;
    let x = to.loc.x + (rel(loc.x, from.loc.x, from.size.w) * to.size.w as f64).round() as i32;
    let y = to.loc.y + (rel(loc.y, from.loc.y, from.size.h) * to.size.h as f64).round() as i32;
    let x = x.min(to.loc.x + to.size.w - size.0).max(to.loc.x);
    let y = y.min(to.loc.y + to.size.h - size.1).max(to.loc.y);
    (x, y).into()
}

impl WhiteRose {
    /// The focused window to the monitor on the `dir` side of its own.
    pub fn move_focused_to_output(&mut self, dir: Dir) {
        let Some(w) = self.focused_window() else { return };
        if self.scratchpads.is_pad(&w) {
            return;
        }
        let Some(from) = self.output_of(&w) else { return };
        let outputs: Vec<Output> = self.space.outputs().cloned().collect();
        let rects: Vec<Rectangle<i32, Logical>> =
            outputs.iter().filter_map(|o| self.space.output_geometry(o)).collect();
        let Some(from_rect) = self.space.output_geometry(&from) else { return };
        let Some(to) = toward(from_rect, &rects, dir).and_then(|i| outputs.get(i).cloned()) else { return };
        self.move_window_to_output(&w, &from, &to);
        self.focus_window(Some(w));
    }

    fn move_window_to_output(&mut self, w: &Window, from: &Output, to: &Output) {
        let (from_area, to_area) = (self.usable_area_on(from), self.usable_area_on(to));
        let size = (w.geometry().size.w, w.geometry().size.h);
        let loc = self.space.element_location(w).unwrap_or(from_area.loc);
        if Self::has_state(w, ToplevelState::Fullscreen) {
            self.fullscreen_to(w, Some(to.clone()));
            return;
        }
        // Where it floats (before it was tiled, maximized or snapped) moves
        // with it.
        let carry = |r: Rectangle<i32, Logical>| {
            Rectangle::new(carried(r.loc, (r.size.w, r.size.h), from_area, to_area), r.size)
        };
        for (f, r) in self.tiling.floating_at.iter_mut().chain(self.restore_geometry.iter_mut()) {
            if f == w {
                *r = carry(*r);
            }
        }
        if Self::has_state(w, ToplevelState::Maximized) {
            self.fill(w, to.clone());
            self.apply(w, ToplevelState::Maximized, true, Some(to_area));
            self.retile();
            return;
        }
        if self.is_tiled(w) {
            self.leave_group(w);
            for s in &mut self.tiling.sets {
                s.tiling.remove(w);
            }
            // On the other screen, so the tiles there take it in.
            self.space.map_element(w.clone(), carried(loc, size, from_area, to_area), false);
            self.retile();
            return;
        }
        if let Some(before) = self.snap.snapped.iter().find(|s| s.window == *w).map(|s| s.before) {
            // Unsnapped: its own size again, where it was relative to the screen.
            let at = carried(loc, (before.size.w, before.size.h), from_area, to_area);
            self.unsnap(w, Some(at));
        } else {
            self.space.map_element(w.clone(), carried(loc, size, from_area, to_area), false);
        }
        self.retile();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i32, y: i32, w: i32, h: i32) -> Rectangle<i32, Logical> {
        Rectangle::new((x, y).into(), (w, h).into())
    }

    #[test]
    fn finds_the_monitor_on_that_side() {
        // A laptop with one monitor to its right, one above that.
        let screens = [r(0, 0, 1440, 900), r(1440, 0, 2560, 1440), r(1440, -1440, 2560, 1440)];
        assert_eq!(toward(screens[0], &screens, Dir::Right), Some(1));
        assert_eq!(toward(screens[0], &screens, Dir::Left), None);
        assert_eq!(toward(screens[0], &screens, Dir::Down), None);
        assert_eq!(toward(screens[1], &screens, Dir::Left), Some(0));
        assert_eq!(toward(screens[1], &screens, Dir::Up), Some(2));
        assert_eq!(toward(screens[2], &screens, Dir::Down), Some(1));
        // Up from the laptop: the one above is to its side, not lined up;
        // it's still the only one that way.
        assert_eq!(toward(screens[0], &screens, Dir::Up), Some(2));
    }

    #[test]
    fn lined_up_beats_nearer_but_off_to_the_side() {
        let me = r(0, 0, 1000, 1000);
        // Further but level with it, nearer but entirely below its bottom.
        let screens = [me, r(1500, 0, 1000, 1000), r(1000, 1200, 1000, 1000)];
        assert_eq!(toward(me, &screens, Dir::Right), Some(1));
    }

    #[test]
    fn keeps_its_place_relative_to_the_screen() {
        let (a, b) = (r(0, 26, 1440, 874), r(1440, 26, 2880, 1748));
        // A quarter in, a quarter down: the same on the bigger screen.
        assert_eq!(carried((360, 26 + 437).into(), (400, 300), a, b), (1440 + 720, 26 + 874).into());
        // Kept inside: a window near the right edge of a big screen on a small one.
        assert_eq!(carried((4000, 26).into(), (800, 600), b, a), (1440 - 800, 26).into());
    }
}
