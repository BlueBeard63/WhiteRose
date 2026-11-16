//! Activities (`activities.enabled`; Figma "Activities - SUPER + SHIFT +
//! A", "Settings - Activities"): each is its own nine workspaces with
//! their tile layouts, and its own dock pins, wallpaper and per-workspace
//! layout and snap-zone overrides (those last two are the compositor's;
//! the shell does the pins and wallpaper).
//!
//! The current activity's workspaces are `WhiteRose::workspaces` and its
//! tiles `tiling.sets`, as before activities, so nothing else needs to
//! know about them. The others' are put aside (`Stashed`) and swapped in
//! when one becomes current. A window on every activity (`everywhere`)
//! is carried across, onto the same workspace, at each switch.
//!
//! Settings: `activities.list` (ids and names, "default" first),
//! `activities.current` (written here, so a restart comes back to it), and
//! `activities.overrides` ({id: {key: value}}): an activity uses the
//! default activity's value of a per-activity key (the plain key, as in
//! 0.3) until it has its own.

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use smithay::desktop::Window;

use crate::{state::WhiteRose, tiling::TileSet, workspace::Workspaces};

/// The activity that's there from the start: the 0.3 settings are its.
pub const DEFAULT: &str = "default";

/// Keys with a value per activity. The compositor applies the two
/// `windows.*` ones; the shell the rest.
pub const PER_ACTIVITY: [&str; 4] =
    ["dock.pinned", "wallpaper.current", "windows.layout.workspaces", "windows.snap.zones.workspaces"];
const COMPOSITOR_KEYS: [&str; 2] = ["windows.layout.workspaces", "windows.snap.zones.workspaces"];

/// An activity that isn't the current one: its workspaces and tiles.
#[derive(Debug)]
pub struct Stashed {
    pub id: String,
    pub workspaces: Workspaces<Window>,
    pub sets: Vec<TileSet>,
}

#[derive(Debug)]
pub struct Activities {
    pub enabled: bool,
    pub current: String,
    /// Every activity's id, in the order `activities.list` has them, and
    /// that list itself.
    pub order: Vec<String>,
    pub entries: Vec<Entry>,
    pub stashed: Vec<Stashed>,
    /// Windows on every activity.
    pub everywhere: Vec<Window>,
    /// `activities.overrides`.
    pub overrides: Map<String, Value>,
    /// The plain (default activity's) values of `COMPOSITOR_KEYS`.
    base: Map<String, Value>,
    /// The picker (SUPER+SHIFT+A): the card chosen (one past the last:
    /// a new activity), and whether SUPER was held when it opened (then
    /// letting go of SUPER switches).
    pub picker: Option<(usize, bool)>,
}

impl Default for Activities {
    fn default() -> Self {
        Activities {
            enabled: false,
            current: DEFAULT.into(),
            order: vec![DEFAULT.into()],
            entries: Vec::new(),
            stashed: Vec::new(),
            everywhere: Vec::new(),
            overrides: Map::new(),
            base: Map::new(),
            picker: None,
        }
    }
}

/// One entry of `activities.list`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub id: String,
    pub name: String,
}

/// An activity as the shell's picker shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActivityView {
    pub id: String,
    /// How many windows are on it (not counting those on every activity).
    pub windows: usize,
    /// Its shown workspace's windows, each as a share of its output: x,
    /// y, width, height from 0 to 1.
    pub places: Vec<[f64; 4]>,
}

impl Activities {
    /// The id of the activity `n` along from the current one (wrapping).
    pub fn along(&self, n: isize) -> Option<&str> {
        let i = self.order.iter().position(|id| *id == self.current)? as isize;
        let len = self.order.len() as isize;
        self.order.get((i + n).rem_euclid(len) as usize).map(String::as_str)
    }

    /// The value of per-activity key `key` for the current activity:
    /// its own, else the plain key's.
    fn effective(&self, key: &str) -> Option<Value> {
        if self.current != DEFAULT {
            if let Some(v) = self.overrides.get(&self.current).and_then(|o| o.get(key)) {
                return Some(v.clone());
            }
        }
        self.base.get(key).cloned()
    }

    /// A name for a new activity: "Activity N", the first N not taken.
    pub fn new_name(names: &[String]) -> String {
        (2..).map(|n| format!("Activity {n}")).find(|n| !names.contains(n)).expect("some number is free")
    }
}

impl WhiteRose {
    /// Takes `activities.*`. A per-activity key (`windows.layout.workspaces`,
    /// `windows.snap.zones.workspaces`) comes back as the value to apply:
    /// the current activity's own, if it has one.
    pub fn activities_setting(&mut self, key: &str, value: &Value) -> Option<Value> {
        match key {
            "activities.enabled" => {
                let on = value.as_bool().unwrap_or(false);
                if !on {
                    // Off: back to the default activity, with every window.
                    self.switch_activity(DEFAULT);
                    let all: Vec<String> = self.activities.stashed.iter().map(|s| s.id.clone()).collect();
                    for id in all {
                        self.absorb_activity(&id);
                    }
                    self.activities.everywhere.clear();
                    self.activities.picker = None;
                }
                self.activities.enabled = on;
                self.activities_sync();
                None
            }
            "activities.list" => {
                let list: Vec<Entry> = serde_json::from_value(value.clone()).unwrap_or_default();
                let mut order: Vec<String> = list.iter().map(|e| e.id.clone()).collect();
                self.activities.entries = list;
                if !order.iter().any(|id| id == DEFAULT) {
                    order.insert(0, DEFAULT.into());
                }
                self.activities.order = order;
                // One that's gone: its windows come to the current one (if
                // that's the one gone, to the default first).
                if !self.activities.order.contains(&self.activities.current) {
                    let gone = self.activities.current.clone();
                    self.switch_activity(DEFAULT);
                    self.absorb_activity(&gone);
                }
                let gone: Vec<String> = self
                    .activities
                    .stashed
                    .iter()
                    .map(|s| s.id.clone())
                    .filter(|id| !self.activities.order.contains(id))
                    .collect();
                for id in gone {
                    self.absorb_activity(&id);
                }
                self.activities_sync();
                None
            }
            // At start-up (nothing open yet), the one it was on.
            "activities.current" => {
                let id = value.as_str().unwrap_or(DEFAULT);
                if self.workspaces.mru().is_empty() && self.activities.stashed.is_empty() {
                    self.activities.current = id.to_owned();
                    self.reapply_activity_settings();
                    self.ipc_sync();
                }
                None
            }
            "activities.overrides" => {
                self.activities.overrides = value.as_object().cloned().unwrap_or_default();
                self.reapply_activity_settings();
                None
            }
            k if COMPOSITOR_KEYS.contains(&k) => {
                self.activities.base.insert(k.to_owned(), value.clone());
                self.activities.effective(k)
            }
            _ => None,
        }
    }

    /// Applies the current activity's per-activity `windows.*` values.
    fn reapply_activity_settings(&mut self) {
        let mut changed = false;
        for k in COMPOSITOR_KEYS {
            let v = self.activities.effective(k).unwrap_or_else(|| Value::Object(Map::new()));
            changed |= self.layout_setting(k, &v);
        }
        if changed {
            self.apply_layout_settings();
        }
    }

    /// Saves `key` (a per-activity key) as the current activity's: the
    /// plain key for the default activity, else its override.
    pub(crate) fn save_activity_setting(&mut self, key: &str, value: Value) {
        if self.activities.current == DEFAULT || !self.activities.enabled {
            self.daemon_request(serde_json::json!({"op": "set", "key": key, "value": value}));
            return;
        }
        let entry = self
            .activities
            .overrides
            .entry(self.activities.current.clone())
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(o) = entry.as_object_mut() {
            o.insert(key.to_owned(), value);
        }
        let all = Value::Object(self.activities.overrides.clone());
        self.daemon_request(serde_json::json!({"op": "set", "key": "activities.overrides", "value": all}));
    }

    /// Makes activity `id` the current one: the windows shown go, its
    /// shown workspace's come, a window on every activity comes along to
    /// the same workspace. One not seen yet starts empty.
    pub fn switch_activity(&mut self, id: &str) {
        if id == self.activities.current {
            return;
        }
        let carried: Vec<(Window, usize)> = self
            .activities
            .everywhere
            .iter()
            .filter_map(|w| Some((w.clone(), self.workspaces.workspace_of(w)?)))
            .collect();
        for w in self.workspaces.active_windows().to_vec() {
            self.hide(&w);
        }
        for (w, _) in &carried {
            self.workspaces.remove(w);
        }
        let next = match self.activities.stashed.iter().position(|s| s.id == id) {
            Some(i) => self.activities.stashed.swap_remove(i),
            None => {
                Stashed { id: id.to_owned(), workspaces: Workspaces::new(crate::state::WORKSPACES), sets: Vec::new() }
            }
        };
        let old_ws = std::mem::replace(&mut self.workspaces, next.workspaces);
        let old_sets = std::mem::replace(&mut self.tiling.sets, next.sets);
        let old = std::mem::replace(&mut self.activities.current, next.id);
        self.activities.stashed.push(Stashed { id: old, workspaces: old_ws, sets: old_sets });
        for (w, ws) in carried {
            self.workspaces.put(w, ws);
        }
        self.show_active_workspace();
        self.workspace_slide = None;
        self.tiling.strips_workspace = None;
        self.reapply_activity_settings();
        self.retile();
        let focus = self.scratchpads.focus.clone().or_else(|| self.next_focus());
        self.focus_window(focus);
        let current = self.activities.current.clone();
        self.daemon_request(serde_json::json!({"op": "set", "key": "activities.current", "value": current}));
        self.ipc_sync();
        self.activities_sync();
    }

    /// Shows the shown workspace's windows (not minimised ones, nor a
    /// group's hidden tabs).
    fn show_active_workspace(&mut self) {
        for w in self.workspaces.active_windows().to_vec() {
            if !self.minimized.contains(&w)
                && !self.tiling.groups.is_hidden(&w)
                && self.space.element_location(&w).is_none()
            {
                self.show(&w);
            }
        }
    }

    /// Activity `id` (not the current one) goes: its windows come to the
    /// current one, on the same workspaces.
    fn absorb_activity(&mut self, id: &str) {
        let Some(i) = self.activities.stashed.iter().position(|s| s.id == id) else { return };
        let gone = self.activities.stashed.swap_remove(i);
        for ws in 0..gone.workspaces.count() {
            for w in gone.workspaces.windows(ws) {
                self.workspaces.put(w.clone(), ws);
            }
        }
        self.show_active_workspace();
        self.retile();
        self.ipc_sync();
    }

    /// Which activity `w` is on (None: the current one, or none at all).
    pub fn stashed_activity_of(&self, w: &Window) -> Option<&str> {
        self.activities.stashed.iter().find(|s| s.workspaces.workspace_of(w).is_some()).map(|s| s.id.as_str())
    }

    /// Puts `w` on activity `to` (None: on every activity). Put on
    /// another one, it goes from here to the same workspace there.
    pub fn set_window_activity(&mut self, w: &Window, to: Option<&str>) {
        if !self.activities.enabled || self.stashed_activity_of(w).is_some() {
            return;
        }
        self.activities.everywhere.retain(|x| x != w);
        let Some(to) = to else {
            self.activities.everywhere.push(w.clone());
            self.activities_sync();
            return;
        };
        if to == self.activities.current || !self.activities.order.iter().any(|id| id == to) {
            self.activities_sync();
            return;
        }
        let Some(ws) = self.workspaces.workspace_of(w) else { return };
        self.leave_group(w);
        self.end_snap(w);
        self.hide(w);
        self.workspaces.remove(w);
        let stash = match self.activities.stashed.iter().position(|s| s.id == to) {
            Some(i) => &mut self.activities.stashed[i],
            None => {
                self.activities.stashed.push(Stashed {
                    id: to.to_owned(),
                    workspaces: Workspaces::new(crate::state::WORKSPACES),
                    sets: Vec::new(),
                });
                self.activities.stashed.last_mut().expect("just pushed")
            }
        };
        stash.workspaces.put(w.clone(), ws);
        self.retile();
        if self.focused_window().as_ref() == Some(w) || self.focused_window().is_none() {
            let next = self.next_focus();
            self.focus_window(next);
        }
        self.ipc_sync();
        self.activities_sync();
    }

    /// `w` closed: forgotten by the activities put aside too.
    pub(crate) fn forget_in_activities(&mut self, w: &Window) {
        self.activities.everywhere.retain(|x| x != w);
        for s in &mut self.activities.stashed {
            s.workspaces.remove(w);
            for set in &mut s.sets {
                set.tiling.remove(w);
            }
        }
    }

    /// Focusing `w` on an activity put aside: that one first.
    pub(crate) fn activity_for_focus(&mut self, w: &Window) {
        if let Some(id) = self.stashed_activity_of(w).map(str::to_owned) {
            self.switch_activity(&id);
        }
    }

    /// Makes a new activity, named the first "Activity N" free, and goes
    /// there. It's saved to `activities.list`.
    pub fn new_activity(&mut self) {
        if !self.activities.enabled {
            return;
        }
        let id = format!(
            "a{:x}",
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_millis())
        );
        let names: Vec<String> = self.activities.entries.iter().map(|e| e.name.clone()).collect();
        let mut list = self.activities.entries.clone();
        if list.is_empty() {
            list.push(Entry { id: DEFAULT.into(), name: "Main".into() });
        }
        list.push(Entry { id: id.clone(), name: Activities::new_name(&names) });
        self.activities.entries = list.clone();
        self.activities.order.push(id.clone());
        self.daemon_request(serde_json::json!({"op": "set", "key": "activities.list", "value": list}));
        self.switch_activity(&id);
    }

    /// Each activity for the shell: its windows, and where its shown
    /// workspace's are.
    pub fn activities_view(&self) -> Vec<ActivityView> {
        let view = |id: &str, ws: &Workspaces<Window>, here: bool| {
            let shown = ws.active_windows();
            let places = shown
                .iter()
                .filter(|w| !self.minimized.contains(w))
                .filter_map(|w| {
                    let g = if here { self.space.element_geometry(w)? } else { self.place_of(w)? };
                    let o = self.space.outputs().find_map(|o| {
                        let og = self.space.output_geometry(o)?;
                        og.contains(g.loc + smithay::utils::Point::from((g.size.w / 2, g.size.h / 2))).then_some(og)
                    })?;
                    let (ow, oh) = (o.size.w.max(1) as f64, o.size.h.max(1) as f64);
                    Some([
                        (g.loc.x - o.loc.x) as f64 / ow,
                        (g.loc.y - o.loc.y) as f64 / oh,
                        g.size.w as f64 / ow,
                        g.size.h as f64 / oh,
                    ])
                })
                .collect();
            let windows = ws.mru().iter().filter(|w| !self.activities.everywhere.contains(w)).count();
            ActivityView { id: id.to_owned(), windows, places }
        };
        self.activities
            .order
            .iter()
            .map(|id| match self.activities.stashed.iter().find(|s| s.id == *id) {
                _ if *id == self.activities.current => view(id, &self.workspaces, true),
                Some(s) => view(id, &s.workspaces, false),
                None => ActivityView { id: id.clone(), windows: 0, places: Vec::new() },
            })
            .collect()
    }

    /// SUPER+SHIFT+A: opens the picker on the next activity (held SUPER
    /// keeps it open; letting go switches), or moves it on.
    pub fn activity_picker(&mut self) {
        if !self.activities.enabled || self.is_locked() {
            return;
        }
        let cards = self.activities.order.len() + 1;
        self.activities.picker = Some(match self.activities.picker {
            Some((i, held)) => ((i + 1) % cards, held),
            None => {
                let here = self.activities.order.iter().position(|id| *id == self.activities.current).unwrap_or(0);
                // Only one: the new-activity card.
                ((here + 1) % self.activities.order.len().max(1) + usize::from(cards == 2), self.shortcut_mods.logo)
            }
        });
        self.activities_sync();
    }

    /// The picker's card `dir` along (wrapping).
    pub fn activity_picker_step(&mut self, dir: isize) {
        let cards = self.activities.order.len() as isize + 1;
        if let Some((i, _)) = self.activities.picker.as_mut() {
            *i = (*i as isize + dir).rem_euclid(cards) as usize;
        }
        self.activities_sync();
    }

    /// Goes to the picker's chosen activity (or makes a new one) and
    /// closes it.
    pub fn commit_activity_picker(&mut self) {
        let Some((i, _)) = self.activities.picker.take() else { return };
        match self.activities.order.get(i).cloned() {
            Some(id) => self.switch_activity(&id),
            None => self.new_activity(),
        }
        self.activities_sync();
    }

    /// What the shell's told.
    pub fn activities_event(&self) -> crate::ipc::Event {
        crate::ipc::Event::Activities {
            enabled: self.activities.enabled,
            current: self.activities.current.clone(),
            everywhere: self.activities.everywhere.iter().map(|w| self.window_id(w)).collect(),
            picker: self.activities.picker.map(|(i, _)| i),
            activities: if self.activities.enabled { self.activities_view() } else { Vec::new() },
        }
    }

    /// Tells the shell (only when it changed).
    pub fn activities_sync(&mut self) {
        let e = self.activities_event();
        if let Some(server) = self.ipc.as_mut() {
            server.update(e);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn along_wraps() {
        let a = Activities {
            order: vec!["default".into(), "b".into(), "c".into()],
            current: "c".into(),
            ..Default::default()
        };
        assert_eq!(a.along(1), Some("default"));
        assert_eq!(a.along(-1), Some("b"));
        assert_eq!(a.along(0), Some("c"));
    }

    #[test]
    fn an_activity_uses_the_defaults_until_it_has_its_own() {
        let mut a = Activities::default();
        a.base.insert("windows.layout.workspaces".into(), serde_json::json!({"2": "master"}));
        assert_eq!(a.effective("windows.layout.workspaces"), Some(serde_json::json!({"2": "master"})));
        a.current = "b".into();
        assert_eq!(a.effective("windows.layout.workspaces"), Some(serde_json::json!({"2": "master"})), "the default's");
        a.overrides.insert("b".into(), serde_json::json!({"windows.layout.workspaces": {}}));
        assert_eq!(a.effective("windows.layout.workspaces"), Some(serde_json::json!({})), "its own");
    }

    #[test]
    fn new_names_skip_ones_taken() {
        assert_eq!(Activities::new_name(&[]), "Activity 2");
        assert_eq!(Activities::new_name(&["Main".into()]), "Activity 2");
        assert_eq!(Activities::new_name(&["Activity 2".into(), "Activity 3".into()]), "Activity 4");
    }
}
