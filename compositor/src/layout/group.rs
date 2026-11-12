//! Grouped (tabbed) windows: several windows share one tile and show one
//! at a time, with a tab strip above it.
//!
//! The tiling tree holds each group's shown window; the other tabs are
//! hidden (like windows on another workspace). Changing the shown tab is a
//! [`super::Tiling::replace`] of the old one with the new one.

/// The height of the tab strip above a grouped tile (tab_strip.rs: a 6 px
/// band, a 30 px island under it, 8 px to the window).
pub const TAB_STRIP: i32 = 44;
/// The band at the top of any tile where a dropped window joins its group
/// (kept as it was when the strip grew, so dropping above still splits).
pub const GROUP_DROP: i32 = 28;

#[derive(Debug, Clone, PartialEq)]
struct Group<W> {
    /// In tab order.
    tabs: Vec<W>,
    shown: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Groups<W> {
    groups: Vec<Group<W>>,
}

impl<W> Default for Groups<W> {
    fn default() -> Self {
        Groups { groups: Vec::new() }
    }
}

impl<W: Clone + PartialEq> Groups<W> {
    fn index(&self, w: &W) -> Option<usize> {
        self.groups.iter().position(|g| g.tabs.contains(w))
    }

    /// The tabs of `w`'s group, if it's in one.
    pub fn tabs_of(&self, w: &W) -> Option<&[W]> {
        self.index(w).map(|i| self.groups[i].tabs.as_slice())
    }

    /// The shown tab of `w`'s group.
    pub fn shown_of(&self, w: &W) -> Option<&W> {
        self.index(w).map(|i| &self.groups[i].tabs[self.groups[i].shown])
    }

    /// In a group, and not the tab that's shown.
    pub fn is_hidden(&self, w: &W) -> bool {
        self.shown_of(w).is_some_and(|s| s != w)
    }

    /// Every group: its tabs and the shown one.
    pub fn iter(&self) -> impl Iterator<Item = (&[W], &W)> {
        self.groups.iter().map(|g| (g.tabs.as_slice(), &g.tabs[g.shown]))
    }

    /// Adds `w` to `target`'s group (making one if `target` is on its own),
    /// just after `target`'s tab, and shows it. Returns the tab that was
    /// shown, which the caller hides. None (and nothing changes) if `w` is
    /// `target` or already in a group.
    pub fn join(&mut self, w: W, target: &W) -> Option<W> {
        if w == *target || self.index(&w).is_some() {
            return None;
        }
        let i = match self.index(target) {
            Some(i) => i,
            None => {
                self.groups.push(Group { tabs: vec![target.clone()], shown: 0 });
                self.groups.len() - 1
            }
        };
        let g = &mut self.groups[i];
        let old = g.tabs[g.shown].clone();
        let at = g.tabs.iter().position(|t| t == target).unwrap_or(g.tabs.len() - 1) + 1;
        g.tabs.insert(at, w);
        g.shown = at;
        Some(old)
    }

    /// Takes `w` out of its group; a group left with one window ends. If
    /// `w` was shown, returns the tab shown in its place (the next one, or
    /// the previous if it was last), which the caller shows.
    pub fn leave(&mut self, w: &W) -> Option<W> {
        let i = self.index(w)?;
        let g = &mut self.groups[i];
        let at = g.tabs.iter().position(|t| t == w)?;
        let was_shown = g.shown == at;
        g.tabs.remove(at);
        if g.shown > at || g.shown == g.tabs.len() {
            g.shown = g.shown.saturating_sub(1);
        }
        let next = g.tabs[g.shown].clone();
        if g.tabs.len() < 2 {
            self.groups.remove(i);
        }
        was_shown.then_some(next)
    }

    /// Shows `w`. Returns the tab that was shown, if that changed.
    pub fn show(&mut self, w: &W) -> Option<W> {
        let i = self.index(w)?;
        let g = &mut self.groups[i];
        let at = g.tabs.iter().position(|t| t == w)?;
        if at == g.shown {
            return None;
        }
        let old = std::mem::replace(&mut g.shown, at);
        Some(g.tabs[old].clone())
    }

    /// The tab after (or before) the shown one in `w`'s group, wrapping.
    pub fn neighbour_tab(&self, w: &W, forward: bool) -> Option<W> {
        let g = &self.groups[self.index(w)?];
        let n = g.tabs.len();
        let at = if forward { (g.shown + 1) % n } else { (g.shown + n - 1) % n };
        Some(g.tabs[at].clone())
    }

    /// Ends `w`'s group. Returns its hidden tabs, which the caller shows.
    pub fn dissolve(&mut self, w: &W) -> Vec<W> {
        let Some(i) = self.index(w) else { return Vec::new() };
        let g = self.groups.remove(i);
        let shown = g.tabs[g.shown].clone();
        g.tabs.into_iter().filter(|t| *t != shown).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn joining_makes_a_group_and_shows_the_newcomer() {
        let mut g = Groups::default();
        assert_eq!(g.join(2, &1), Some(1), "1 was shown: hide it");
        assert_eq!(g.tabs_of(&1), Some(&[1, 2][..]));
        assert_eq!(g.shown_of(&1), Some(&2));
        assert!(g.is_hidden(&1) && !g.is_hidden(&2) && !g.is_hidden(&9));
        // A third joins after the shown tab.
        assert_eq!(g.join(3, &2), Some(2));
        assert_eq!(g.tabs_of(&3), Some(&[1, 2, 3][..]));
        assert_eq!(g.iter().count(), 1);
    }

    #[test]
    fn join_ignores_bad_input() {
        let mut g = Groups::default();
        assert_eq!(g.join(1, &1), None);
        g.join(2, &1);
        assert_eq!(g.join(2, &5), None, "2 is already grouped");
        assert_eq!(g.tabs_of(&5), None);
    }

    #[test]
    fn leaving_shows_the_next_tab_and_ends_small_groups() {
        let mut g = Groups::default();
        g.join(2, &1);
        g.join(3, &2);
        g.show(&2);
        // A hidden tab leaves: nothing to show.
        assert_eq!(g.leave(&1), None);
        assert_eq!(g.tabs_of(&2), Some(&[2, 3][..]));
        assert_eq!(g.shown_of(&2), Some(&2));
        // The shown one leaves: the other is shown, and the group ends.
        assert_eq!(g.leave(&2), Some(3));
        assert_eq!(g.tabs_of(&3), None);
        assert_eq!(g.leave(&3), None, "not grouped");
    }

    #[test]
    fn the_last_tab_leaving_shows_the_one_before() {
        let mut g = Groups::default();
        g.join(2, &1);
        g.join(3, &2);
        assert_eq!(g.leave(&3), Some(2));
        assert_eq!(g.shown_of(&1), Some(&2));
    }

    #[test]
    fn show_and_neighbour_tabs() {
        let mut g = Groups::default();
        g.join(2, &1);
        g.join(3, &2);
        assert_eq!(g.show(&1), Some(3));
        assert_eq!(g.show(&1), None, "already shown");
        assert_eq!(g.neighbour_tab(&1, true), Some(2));
        assert_eq!(g.neighbour_tab(&1, false), Some(3), "wraps");
        assert_eq!(g.show(&9), None);
        assert_eq!(g.neighbour_tab(&9, true), None);
    }

    #[test]
    fn dissolve_returns_the_hidden_tabs() {
        let mut g = Groups::default();
        g.join(2, &1);
        g.join(3, &2);
        let mut hidden = g.dissolve(&1);
        hidden.sort();
        assert_eq!(hidden, vec![1, 2]);
        assert_eq!(g.iter().count(), 0);
        assert!(g.dissolve(&1).is_empty());
    }
}
