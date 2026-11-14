//! Folders' own colours and icons, in $XDG_STATE_HOME/whiterose/files/looks.json:
//! a colour from the palette, a symbol drawn on the folder, or a picture of
//! your own. They follow a folder that Files renames, moves, trashes or
//! restores, and go with one deleted for good.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

/// The colours offered (the folder drawn in it).
pub const COLOURS: &[&str] = &[
    "rose", "red", "orange", "yellow", "green", "teal", "blue", "purple", "pink", "grey",
];

/// The symbols offered (kit Icon names).
pub const SYMBOLS: &[&str] = &[
    "star",
    "heart",
    "code",
    "music",
    "image",
    "camera",
    "film",
    "book",
    "briefcase",
    "download",
    "home",
    "school",
    "globe",
    "cloud",
    "lock",
    "gear",
    "terminal",
    "calendar",
    "user",
    "package",
    "leaf",
    "zap",
    "shield",
    "palette",
];

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Look {
    /// One of COLOURS, or "" (the theme's folder).
    pub colour: String,
    /// One of SYMBOLS, or "".
    pub symbol: String,
    /// A picture used as the folder's icon (an absolute path), or "".
    pub image: String,
}

impl Look {
    fn is_empty(&self) -> bool {
        self.colour.is_empty() && self.symbol.is_empty() && self.image.is_empty()
    }

    /// Only what's offered (anything else is dropped).
    pub fn checked(self) -> Look {
        Look {
            colour: if COLOURS.contains(&self.colour.as_str()) {
                self.colour
            } else {
                String::new()
            },
            symbol: if SYMBOLS.contains(&self.symbol.as_str()) {
                self.symbol
            } else {
                String::new()
            },
            image: if self.image.starts_with('/') && self.image.len() < 4096 {
                self.image
            } else {
                String::new()
            },
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Looks {
    pub folders: BTreeMap<String, Look>,
}

impl Looks {
    pub fn get(&self, path: &str) -> Option<&Look> {
        self.folders.get(path)
    }

    pub fn set(&mut self, path: &str, look: Look) {
        let look = look.checked();
        if look.is_empty() {
            self.folders.remove(path);
        } else {
            self.folders.insert(path.to_string(), look);
        }
    }

    /// `from` (and folders inside it) is now at `to`.
    pub fn moved(&mut self, from: &str, to: &str) -> bool {
        let inside = format!("{from}/");
        let keys: Vec<String> = self
            .folders
            .keys()
            .filter(|k| *k == from || k.starts_with(&inside))
            .cloned()
            .collect();
        for k in &keys {
            if let Some(l) = self.folders.remove(k) {
                self.folders.insert(format!("{to}{}", &k[from.len()..]), l);
            }
        }
        !keys.is_empty()
    }

    /// `path` (and folders inside it) is gone for good.
    pub fn forget(&mut self, path: &str) -> bool {
        let inside = format!("{path}/");
        let before = self.folders.len();
        self.folders
            .retain(|k, _| k != path && !k.starts_with(&inside));
        self.folders.len() != before
    }
}

pub fn file() -> PathBuf {
    crate::session::dir().join("looks.json")
}

pub fn load_from(file: &Path) -> Looks {
    std::fs::read_to_string(file)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

fn save_to(file: &Path, l: &Looks) -> Result<(), String> {
    std::fs::create_dir_all(file.parent().ok_or("no folder")?).map_err(|e| e.to_string())?;
    let tmp = file.with_extension("json.tmp");
    std::fs::write(&tmp, serde_json::to_string(l).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, file).map_err(|e| e.to_string())
}

pub fn load() -> Looks {
    load_from(&file())
}

/// Loads, changes and saves if `f` changed something (read fresh: other
/// Files processes may have changed it).
pub fn change(f: impl FnOnce(&mut Looks) -> bool) -> bool {
    let path = file();
    let mut l = load_from(&path);
    if !f(&mut l) {
        return true;
    }
    save_to(&path, &l).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    fn look(colour: &str, symbol: &str) -> Look {
        Look {
            colour: colour.into(),
            symbol: symbol.into(),
            image: String::new(),
        }
    }

    #[test]
    fn set_and_clean() {
        let mut l = Looks::default();
        l.set("/a", look("teal", "code"));
        assert_eq!(l.get("/a"), Some(&look("teal", "code")));
        l.set(
            "/b",
            Look {
                colour: "chartreuse".into(),
                symbol: "rm -rf".into(),
                image: "relative.png".into(),
            },
        );
        assert_eq!(l.get("/b"), None, "nothing offered: nothing kept");
        l.set("/a", Look::default());
        assert!(l.folders.is_empty(), "back to plain: forgotten");
    }

    #[test]
    fn follows_moves_and_deletes() {
        let mut l = Looks::default();
        l.set("/home/me/Work", look("blue", ""));
        l.set("/home/me/Work/Code", look("", "code"));
        l.set("/home/me/Workshop", look("red", ""));
        assert!(l.moved("/home/me/Work", "/home/me/Projects"));
        assert_eq!(
            l.get("/home/me/Projects").map(|x| x.colour.as_str()),
            Some("blue")
        );
        assert_eq!(
            l.get("/home/me/Projects/Code").map(|x| x.symbol.as_str()),
            Some("code"),
            "what's inside goes too"
        );
        assert!(
            l.get("/home/me/Workshop").is_some(),
            "a name that only starts the same stays"
        );
        assert!(!l.moved("/nothing", "/x"));
        assert!(l.forget("/home/me/Projects"));
        assert_eq!(l.folders.len(), 1);
    }

    #[test]
    fn saved_and_loaded() {
        let d = Scratch::new("looks");
        let f = d.join("looks.json");
        assert_eq!(load_from(&f), Looks::default());
        let mut l = Looks::default();
        l.set(
            "/a",
            Look {
                colour: "green".into(),
                symbol: "star".into(),
                image: "/home/me/pic.png".into(),
            },
        );
        save_to(&f, &l).unwrap();
        assert_eq!(load_from(&f), l);
    }
}
