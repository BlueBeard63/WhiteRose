//! ALT+TAB (applications) and SUPER+TAB (windows). Pure selection logic;
//! the compositor feeds it keys and the shell draws it from IPC events.
//!
//! Items are taken from the focus history when the switcher opens, so the
//! order doesn't shuffle under the user's fingers while it's open.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    /// SUPER+TAB: one item per window.
    Windows,
    /// ALT+TAB: one item per application, holding its windows.
    Apps,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Item {
    pub app_id: String,
    /// Window ids, most recently used first.
    pub windows: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Switcher {
    pub mode: Mode,
    pub items: Vec<Item>,
    pub selected: usize,
    /// Which of the selected app's windows (` cycles it in app mode).
    pub sub: usize,
}

impl Switcher {
    /// Opens on the *previous* item, so a quick tap-and-release toggles
    /// between the two most recent windows/apps. `mru` is (window id, app id),
    /// most recent first. Returns None when there is nothing to switch to.
    pub fn open(mode: Mode, mru: &[(u64, String)]) -> Option<Switcher> {
        let items: Vec<Item> = match mode {
            Mode::Windows => mru.iter().map(|(id, app)| Item { app_id: app.clone(), windows: vec![*id] }).collect(),
            Mode::Apps => {
                let mut items: Vec<Item> = Vec::new();
                for (id, app) in mru {
                    match items.iter_mut().find(|i| &i.app_id == app) {
                        Some(i) => i.windows.push(*id),
                        None => items.push(Item { app_id: app.clone(), windows: vec![*id] }),
                    }
                }
                items
            }
        };
        if items.is_empty() {
            return None;
        }
        let selected = if items.len() > 1 { 1 } else { 0 };
        Some(Switcher { mode, items, selected, sub: 0 })
    }

    pub fn next(&mut self) {
        self.selected = (self.selected + 1) % self.items.len();
        self.sub = 0;
    }

    pub fn prev(&mut self) {
        self.selected = (self.selected + self.items.len() - 1) % self.items.len();
        self.sub = 0;
    }

    /// ` in app mode: the next window of the selected app.
    pub fn next_window(&mut self) {
        let n = self.items[self.selected].windows.len();
        self.sub = (self.sub + 1) % n;
    }

    /// The window to focus when the switcher closes.
    pub fn target(&self) -> Option<u64> {
        self.items.get(self.selected).and_then(|i| i.windows.get(self.sub)).copied()
    }

    /// The windows Q should close: one window, or every window of the app.
    pub fn close_targets(&self) -> Vec<u64> {
        match self.mode {
            Mode::Windows => self.target().into_iter().collect(),
            Mode::Apps => self.items.get(self.selected).map(|i| i.windows.clone()).unwrap_or_default(),
        }
    }

    /// A window closed while the switcher was open. Returns false if nothing is left.
    pub fn remove_window(&mut self, id: u64) -> bool {
        for item in &mut self.items {
            item.windows.retain(|w| *w != id);
        }
        let before = self.selected;
        let mut i = 0;
        self.items.retain(|item| {
            let keep = !item.windows.is_empty();
            if !keep && i < before {
                self.selected = self.selected.saturating_sub(1);
            }
            i += 1;
            keep
        });
        if self.items.is_empty() {
            return false;
        }
        self.selected = self.selected.min(self.items.len() - 1);
        self.sub = self.sub.min(self.items[self.selected].windows.len() - 1);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mru() -> Vec<(u64, String)> {
        // Most recent first: term(3), browser(2), term(1), code(4)
        vec![(3, "term".into()), (2, "browser".into()), (1, "term".into()), (4, "code".into())]
    }

    #[test]
    fn window_mode_opens_on_the_previous_window() {
        let s = Switcher::open(Mode::Windows, &mru()).unwrap();
        assert_eq!(s.items.len(), 4);
        assert_eq!(s.target(), Some(2), "quick SUPER+TAB goes back to the last window");
    }

    #[test]
    fn app_mode_groups_by_app_in_recency_order() {
        let s = Switcher::open(Mode::Apps, &mru()).unwrap();
        let apps: Vec<_> = s.items.iter().map(|i| (i.app_id.as_str(), i.windows.clone())).collect();
        assert_eq!(apps, vec![("term", vec![3, 1]), ("browser", vec![2]), ("code", vec![4])]);
        assert_eq!(s.target(), Some(2), "quick ALT+TAB goes back to the last app");
    }

    #[test]
    fn next_and_prev_wrap() {
        let mut s = Switcher::open(Mode::Windows, &mru()).unwrap();
        s.next();
        s.next();
        s.next();
        assert_eq!(s.selected, 0, "wrapped past the end");
        s.prev();
        assert_eq!(s.selected, 3, "wrapped before the start");
    }

    #[test]
    fn backtick_cycles_the_selected_apps_windows() {
        let mut s = Switcher::open(Mode::Apps, &mru()).unwrap();
        s.prev(); // term
        assert_eq!(s.target(), Some(3));
        s.next_window();
        assert_eq!(s.target(), Some(1));
        s.next_window();
        assert_eq!(s.target(), Some(3), "wrapped");
        s.next(); // moving to another app resets the window choice
        assert_eq!(s.sub, 0);
    }

    #[test]
    fn single_item_opens_on_itself() {
        let s = Switcher::open(Mode::Windows, &[(7, "a".into())]).unwrap();
        assert_eq!(s.target(), Some(7));
        assert!(Switcher::open(Mode::Apps, &[]).is_none(), "nothing to switch to");
    }

    #[test]
    fn q_closes_a_window_or_a_whole_app() {
        let w = Switcher::open(Mode::Windows, &mru()).unwrap();
        assert_eq!(w.close_targets(), vec![2]);
        let mut a = Switcher::open(Mode::Apps, &mru()).unwrap();
        a.prev();
        assert_eq!(a.close_targets(), vec![3, 1], "quit app closes all its windows");
    }

    #[test]
    fn removing_windows_keeps_the_selection_sensible() {
        let mut s = Switcher::open(Mode::Windows, &mru()).unwrap();
        s.next(); // selected = 2 (window 1)
        assert!(s.remove_window(3)); // an earlier item went away
        assert_eq!(s.target(), Some(1), "selection followed its window");
        assert!(s.remove_window(1)); // the selected one went away
        assert!(s.target().is_some());
        assert!(s.remove_window(2));
        assert!(!s.remove_window(4), "nothing left");
    }

    #[test]
    fn removing_one_of_an_apps_windows_keeps_the_app() {
        let mut s = Switcher::open(Mode::Apps, &mru()).unwrap();
        s.prev(); // term [3, 1]
        s.next_window(); // window 1
        assert!(s.remove_window(1));
        assert_eq!(s.items[0].windows, vec![3]);
        assert_eq!(s.target(), Some(3));
    }
}
