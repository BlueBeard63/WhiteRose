//! How each folder is shown (grid, list or columns, and the sort), in
//! $XDG_STATE_HOME/whiterose/files/views.json. Changing the view in a folder
//! remembers it for that folder. A folder can lend its view to everything
//! under it ("subfolders"), and a default covers the rest:
//!
//!   the folder's own view → the nearest folder above lending its view → the default
//!
//! Showing hidden files is one setting for everywhere, kept here too.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// Folders remembered at most (the least recently set go first).
const KEEP: usize = 1000;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(rename_all = "camelCase", default)]
pub struct View {
    pub mode: String,
    pub sort_by: String,
    pub descending: bool,
}

impl Default for View {
    fn default() -> Self {
        View {
            mode: "grid".into(),
            sort_by: "name".into(),
            descending: false,
        }
    }
}

impl View {
    fn checked(self) -> View {
        View {
            mode: match self.mode.as_str() {
                "list" | "columns" | "gallery" | "cards" | "usage" => self.mode,
                _ => "grid".into(),
            },
            sort_by: match self.sort_by.as_str() {
                "size" | "modified" | "kind" => self.sort_by,
                _ => "name".into(),
            },
            descending: self.descending,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Folder {
    #[serde(flatten)]
    pub view: View,
    /// Lends its view to the folders under it.
    pub subfolders: bool,
    /// When it was last set (for forgetting the oldest).
    pub stamp: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Views {
    pub default: View,
    pub hidden: bool,
    /// The details panel shown (everywhere).
    pub details: bool,
    /// A folder with no view of its own (and none lent) gets one to suit
    /// what's in it: photos a gallery, and so on.
    pub adaptive: bool,
    /// How big items are drawn (0.75–1.6).
    pub zoom: f64,
    pub folders: BTreeMap<String, Folder>,
    next: u64,
}

impl Default for Views {
    fn default() -> Self {
        Views {
            default: View::default(),
            hidden: false,
            details: false,
            adaptive: true,
            zoom: 1.0,
            folders: BTreeMap::new(),
            next: 0,
        }
    }
}

/// Zoom kept to what's drawable.
pub fn zoom_checked(z: f64) -> f64 {
    if z.is_finite() {
        z.clamp(0.75, 1.6)
    } else {
        1.0
    }
}

/// The view a folder gets, and where it comes from.
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Found {
    #[serde(flatten)]
    pub view: View,
    /// The folder has a view of its own.
    pub own: bool,
    /// The folder lends its view to its subfolders.
    pub subfolders: bool,
    /// The folder above it lent it ("" otherwise).
    pub from: String,
}

impl Views {
    pub fn find(&self, path: &str) -> Found {
        if let Some(f) = self.folders.get(path) {
            return Found {
                view: f.view.clone(),
                own: true,
                subfolders: f.subfolders,
                from: String::new(),
            };
        }
        let mut p = Path::new(path).parent();
        while let Some(up) = p {
            if let Some(f) = self.folders.get(&*up.to_string_lossy()) {
                if f.subfolders {
                    return Found {
                        view: f.view.clone(),
                        own: false,
                        subfolders: false,
                        from: up.to_string_lossy().into_owned(),
                    };
                }
            }
            p = up.parent();
        }
        Found {
            view: self.default.clone(),
            own: false,
            subfolders: false,
            from: String::new(),
        }
    }

    fn touch(&mut self) -> u64 {
        self.next += 1;
        self.next
    }

    /// This folder's view, from now on.
    pub fn set(&mut self, path: &str, view: View) {
        let stamp = self.touch();
        let f = self.folders.entry(path.to_string()).or_default();
        f.view = view.checked();
        f.stamp = stamp;
        self.forget_oldest();
    }

    /// Lend (or stop lending) this folder's view to its subfolders; it gets
    /// a view of its own (what it shows now) if it hadn't one.
    pub fn set_subfolders(&mut self, path: &str, on: bool) {
        let shown = self.find(path).view;
        let stamp = self.touch();
        let f = self
            .folders
            .entry(path.to_string())
            .or_insert_with(|| Folder {
                view: shown,
                ..Default::default()
            });
        f.subfolders = on;
        f.stamp = stamp;
    }

    /// The view for every folder without one; this folder goes back to it.
    pub fn set_default(&mut self, path: &str, view: View) {
        self.default = view.checked();
        if self.folders.get(path).is_some_and(|f| !f.subfolders) {
            self.folders.remove(path);
        }
    }

    /// Back to what it would get without a view of its own.
    pub fn reset(&mut self, path: &str) {
        self.folders.remove(path);
    }

    fn forget_oldest(&mut self) {
        while self.folders.len() > KEEP {
            // Folders lending their view are kept: they were set on purpose.
            let oldest = self
                .folders
                .iter()
                .filter(|(_, f)| !f.subfolders)
                .min_by_key(|(_, f)| f.stamp)
                .map(|(k, _)| k.clone());
            match oldest {
                Some(k) => {
                    self.folders.remove(&k);
                }
                None => break,
            }
        }
    }
}

pub fn file() -> PathBuf {
    crate::session::dir().join("views.json")
}

pub fn load_from(file: &Path) -> Views {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save_to(file: &Path, v: &Views) -> Result<(), String> {
    let dir = file.parent().ok_or("no folder")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string(v).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, file).map_err(|e| e.to_string())
}

/// Loads, changes and saves (read fresh each time: other Files windows in
/// other processes may have changed it).
pub fn change(f: impl FnOnce(&mut Views)) -> bool {
    let path = file();
    let mut v = load_from(&path);
    f(&mut v);
    save_to(&path, &v).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(mode: &str) -> View {
        View {
            mode: mode.into(),
            ..View::default()
        }
    }

    #[test]
    fn own_then_lent_then_default() {
        let mut v = Views::default();
        assert_eq!(v.find("/home/me/Projects").view.mode, "grid");
        v.set("/home/me/Projects", view("columns"));
        let f = v.find("/home/me/Projects");
        assert!(f.own);
        assert_eq!(f.view.mode, "columns");
        assert_eq!(
            v.find("/home/me/Projects/app").view.mode,
            "grid",
            "not lent yet"
        );

        v.set_subfolders("/home/me/Projects", true);
        let f = v.find("/home/me/Projects/app/src");
        assert_eq!(
            (f.view.mode.as_str(), f.own, f.from.as_str()),
            ("columns", false, "/home/me/Projects")
        );

        v.set("/home/me/Projects/app", view("list"));
        assert_eq!(
            v.find("/home/me/Projects/app").view.mode,
            "list",
            "its own beats the lent one"
        );
        assert_eq!(
            v.find("/home/me/Projects/app/src").view.mode,
            "columns",
            "only lenders lend"
        );
        assert_eq!(
            v.find("/home/me/Projects2").view.mode,
            "grid",
            "a sibling with a similar name isn't under it"
        );
    }

    #[test]
    fn nearest_lender_wins() {
        let mut v = Views::default();
        v.set("/a", view("list"));
        v.set_subfolders("/a", true);
        v.set("/a/b", view("columns"));
        v.set_subfolders("/a/b", true);
        assert_eq!(v.find("/a/b/c/d").view.mode, "columns");
        assert_eq!(v.find("/a/x").view.mode, "list");
    }

    #[test]
    fn lending_without_a_view_takes_what_it_shows() {
        let mut v = Views::default();
        v.set("/a", view("list"));
        v.set_subfolders("/a", true);
        v.set_subfolders("/a/b", true);
        assert_eq!(
            v.folders["/a/b"].view.mode, "list",
            "/a/b showed /a's list, and keeps it"
        );
    }

    #[test]
    fn default_and_reset() {
        let mut v = Views::default();
        v.set("/a", view("list"));
        v.set_default("/a", view("columns"));
        assert!(
            !v.folders.contains_key("/a"),
            "the folder goes back to the default"
        );
        assert_eq!(v.find("/anything").view.mode, "columns");
        v.set("/b", view("list"));
        v.reset("/b");
        assert_eq!(v.find("/b").view.mode, "columns");
    }

    #[test]
    fn bad_values_are_cleaned_and_oldest_forgotten() {
        let mut v = Views::default();
        v.set(
            "/x",
            View {
                mode: "huge".into(),
                sort_by: "colour".into(),
                descending: true,
            },
        );
        assert_eq!(
            v.find("/x").view,
            View {
                mode: "grid".into(),
                sort_by: "name".into(),
                descending: true
            }
        );
        v.set_subfolders("/keep", true);
        for i in 0..KEEP + 5 {
            v.set(&format!("/f{i}"), view("list"));
        }
        assert_eq!(v.folders.len(), KEEP);
        assert!(v.folders.contains_key("/keep"), "lenders stay");
        assert!(!v.folders.contains_key("/x") && !v.folders.contains_key("/f0"));
    }

    #[test]
    fn saved_and_loaded() {
        let d = std::env::temp_dir().join(format!("wr-views-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        let f = d.join("views.json");
        assert_eq!(load_from(&f), Views::default());
        let mut v = Views {
            hidden: true,
            ..Views::default()
        };
        v.set("/a", view("columns"));
        save_to(&f, &v).unwrap();
        assert_eq!(load_from(&f), v);
    }

    #[test]
    fn new_views_and_settings() {
        let mut v = Views::default();
        assert!(v.adaptive, "adaptive unless turned off");
        for m in ["gallery", "cards", "usage"] {
            v.set("/a", view(m));
            assert_eq!(v.find("/a").view.mode, m);
        }
        assert_eq!(zoom_checked(9.0), 1.6);
        assert_eq!(zoom_checked(f64::NAN), 1.0);
        let old: Views = serde_json::from_str(r#"{"hidden":true}"#).unwrap();
        assert!(
            old.adaptive && old.zoom == 1.0,
            "older files get the defaults"
        );
    }
}
