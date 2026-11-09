//! Workspaces and focus history. Generic over the window type so the logic
//! is tested without a running compositor.

/// What the compositor must do to apply a workspace switch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Switch<W> {
    pub hide: Vec<W>,
    /// Bottom-to-top stacking order.
    pub show: Vec<W>,
}

#[derive(Debug, Clone)]
pub struct Workspaces<W> {
    active: usize,
    /// Per workspace, windows in stacking order (last is on top).
    stacks: Vec<Vec<W>>,
    /// Every window, most recently focused first. Drives the switchers.
    mru: Vec<W>,
}

impl<W: Clone + PartialEq> Workspaces<W> {
    /// Creates `count` empty workspaces. Panics if `count` is 0.
    pub fn new(count: usize) -> Self {
        assert!(count > 0, "need at least one workspace");
        Workspaces { active: 0, stacks: vec![Vec::new(); count], mru: Vec::new() }
    }

    pub fn count(&self) -> usize {
        self.stacks.len()
    }

    /// Index (0-based) of the visible workspace.
    pub fn active(&self) -> usize {
        self.active
    }

    pub fn windows(&self, ws: usize) -> &[W] {
        self.stacks.get(ws).map(Vec::as_slice).unwrap_or(&[])
    }

    pub fn active_windows(&self) -> &[W] {
        self.windows(self.active)
    }

    pub fn workspace_of(&self, w: &W) -> Option<usize> {
        self.stacks.iter().position(|s| s.contains(w))
    }

    /// Adds a new window on the active workspace, on top and focused.
    /// Adding a window that is already tracked just focuses it.
    pub fn add(&mut self, w: W) {
        if self.workspace_of(&w).is_none() {
            self.stacks[self.active].push(w.clone());
        }
        self.focus(&w);
    }

    /// Forgets a window. Returns the workspace it was on.
    pub fn remove(&mut self, w: &W) -> Option<usize> {
        self.mru.retain(|x| x != w);
        let ws = self.workspace_of(w)?;
        self.stacks[ws].retain(|x| x != w);
        Some(ws)
    }

    /// Records focus: raises the window in its stack and moves it to the
    /// front of the focus history. Unknown windows are ignored.
    pub fn focus(&mut self, w: &W) {
        let Some(ws) = self.workspace_of(w) else { return };
        let stack = &mut self.stacks[ws];
        stack.retain(|x| x != w);
        stack.push(w.clone());
        self.mru.retain(|x| x != w);
        self.mru.insert(0, w.clone());
    }

    /// The most recently focused window on the active workspace.
    pub fn focused(&self) -> Option<&W> {
        self.mru.iter().find(|w| self.workspace_of(w) == Some(self.active))
    }

    /// Switches workspace. Returns None if `ws` is out of range or already active.
    pub fn switch_to(&mut self, ws: usize) -> Option<Switch<W>> {
        if ws >= self.count() || ws == self.active {
            return None;
        }
        let hide = self.stacks[self.active].clone();
        self.active = ws;
        Some(Switch { hide, show: self.stacks[ws].clone() })
    }

    /// Moves a window to another workspace (on top of its stack). Returns
    /// true if the window must now be hidden because that workspace is not
    /// visible. Out-of-range targets and unknown windows are ignored.
    pub fn move_to(&mut self, w: &W, ws: usize) -> bool {
        if ws >= self.count() {
            return false;
        }
        let Some(from) = self.workspace_of(w) else { return false };
        if from == ws {
            return false;
        }
        self.stacks[from].retain(|x| x != w);
        self.stacks[ws].push(w.clone());
        ws != self.active
    }

    /// Every window, most recently focused first (for SUPER+ALT+TAB).
    pub fn mru(&self) -> &[W] {
        &self.mru
    }

    /// Windows grouped by application, apps ordered by their most recently
    /// focused window (for ALT+TAB).
    pub fn mru_by_app<K: PartialEq>(&self, app: impl Fn(&W) -> K) -> Vec<(K, Vec<W>)> {
        let mut groups: Vec<(K, Vec<W>)> = Vec::new();
        for w in &self.mru {
            let k = app(w);
            match groups.iter_mut().find(|(g, _)| *g == k) {
                Some((_, ws)) => ws.push(w.clone()),
                None => groups.push((k, vec![w.clone()])),
            }
        }
        groups
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[should_panic]
    fn zero_workspaces_panics() {
        let _ = Workspaces::<u32>::new(0);
    }

    #[test]
    fn new_windows_go_to_active_workspace_and_take_focus() {
        let mut w = Workspaces::new(3);
        w.add(1);
        w.add(2);
        assert_eq!(w.active_windows(), &[1, 2]);
        assert_eq!(w.focused(), Some(&2));
        assert_eq!(w.mru(), &[2, 1]);
    }

    #[test]
    fn adding_twice_does_not_duplicate() {
        let mut w = Workspaces::new(1);
        w.add(1);
        w.add(2);
        w.add(1);
        assert_eq!(w.active_windows(), &[2, 1]);
        assert_eq!(w.mru(), &[1, 2]);
    }

    #[test]
    fn focus_raises_and_updates_history() {
        let mut w = Workspaces::new(1);
        w.add(1);
        w.add(2);
        w.add(3);
        w.focus(&1);
        assert_eq!(w.active_windows(), &[2, 3, 1]);
        assert_eq!(w.mru(), &[1, 3, 2]);
        w.focus(&99); // unknown: ignored
        assert_eq!(w.mru(), &[1, 3, 2]);
    }

    #[test]
    fn switching_reports_what_to_hide_and_show() {
        let mut w = Workspaces::new(3);
        w.add(1);
        w.add(2);
        let s = w.switch_to(1).unwrap();
        assert_eq!(s, Switch { hide: vec![1, 2], show: vec![] });
        w.add(3);
        let s = w.switch_to(0).unwrap();
        assert_eq!(s, Switch { hide: vec![3], show: vec![1, 2] });
        assert_eq!(w.active(), 0);
    }

    #[test]
    fn switching_to_same_or_missing_workspace_is_a_no_op() {
        let mut w = Workspaces::<u32>::new(2);
        assert_eq!(w.switch_to(0), None);
        assert_eq!(w.switch_to(2), None);
        assert_eq!(w.active(), 0);
    }

    #[test]
    fn focused_is_per_workspace() {
        let mut w = Workspaces::new(2);
        w.add(1);
        w.switch_to(1);
        assert_eq!(w.focused(), None, "empty workspace has no focus");
        w.add(2);
        assert_eq!(w.focused(), Some(&2));
        w.switch_to(0);
        assert_eq!(w.focused(), Some(&1));
    }

    #[test]
    fn move_to_other_workspace_hides_window() {
        let mut w = Workspaces::new(3);
        w.add(1);
        w.add(2);
        assert!(w.move_to(&2, 2));
        assert_eq!(w.windows(0), &[1]);
        assert_eq!(w.windows(2), &[2]);
        assert_eq!(w.workspace_of(&2), Some(2));
        // Moving onto the visible workspace doesn't hide it.
        w.switch_to(2);
        assert!(!w.move_to(&1, 2));
        assert_eq!(w.windows(2), &[2, 1]);
    }

    #[test]
    fn move_to_ignores_bad_input() {
        let mut w = Workspaces::new(2);
        w.add(1);
        assert!(!w.move_to(&1, 5), "out of range");
        assert!(!w.move_to(&1, 0), "already there");
        assert!(!w.move_to(&7, 1), "unknown window");
        assert_eq!(w.windows(0), &[1]);
    }

    #[test]
    fn remove_forgets_everywhere() {
        let mut w = Workspaces::new(2);
        w.add(1);
        w.add(2);
        w.move_to(&1, 1);
        assert_eq!(w.remove(&1), Some(1));
        assert_eq!(w.windows(1), &[] as &[i32]);
        assert_eq!(w.mru(), &[2]);
        assert_eq!(w.remove(&1), None);
    }

    #[test]
    fn mru_by_app_groups_in_recency_order() {
        // (app, window id)
        let mut w = Workspaces::new(2);
        for x in [("browser", 1), ("term", 2), ("browser", 3), ("code", 4)] {
            w.add(x);
        }
        w.focus(&("term", 2));
        // MRU: term2, code4, browser3, browser1
        let groups = w.mru_by_app(|(app, _)| *app);
        let apps: Vec<_> = groups.iter().map(|(a, ws)| (*a, ws.len())).collect();
        assert_eq!(apps, vec![("term", 1), ("code", 1), ("browser", 2)]);
        assert_eq!(groups[2].1, vec![("browser", 3), ("browser", 1)]);
    }
}
