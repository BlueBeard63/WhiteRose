//! The open windows and their tabs, kept between runs in
//! $XDG_STATE_HOME/whiterose/files/session.json. On the way back in, tabs
//! whose folder has gone open the nearest folder that's still there, and
//! history entries that have gone are dropped.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// History kept per tab, each way.
const HISTORY: usize = 50;

/// A tab: its folder and history, and when it's split, the other side's
/// (and which way, and which side was in use). How a folder is shown is the
/// folder's own business: views.rs.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Tab {
    pub path: String,
    pub back: Vec<String>,
    pub forward: Vec<String>,
    pub second: Option<Box<Tab>>,
    /// One above the other rather than side by side.
    pub vertical: bool,
    /// 0 or 1: the side in use.
    pub active: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Window {
    pub tabs: Vec<Tab>,
    pub current: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
#[serde(default)]
pub struct Session {
    pub windows: Vec<Window>,
}

pub fn dir() -> PathBuf {
    let state = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| crate::places::home().join(".local/state"));
    state.join("whiterose/files")
}

/// On a network drive, asking whether a folder exists is a trip to the
/// server each (restoring a history of them took seconds). There, a
/// folder counts while its drive is mounted: known from the mount's own
/// folder (GVfs answers listing its root itself) or mountinfo, never the
/// server.
fn remote_present(p: &str) -> Option<bool> {
    if !crate::places::is_remote(p) {
        return None;
    }
    let run = std::env::var_os("XDG_RUNTIME_DIR").map(std::path::PathBuf::from)?;
    Some(match crate::places::mount_place(p, &run) {
        Some((root, _)) => {
            let root = Path::new(&root);
            root.parent()
                .and_then(|g| std::fs::read_dir(g).ok())
                .is_some_and(|mut d| {
                    d.any(|e| e.is_ok_and(|e| Some(e.file_name().as_os_str()) == root.file_name()))
                })
        }
        // (Some other network mount, NFS or SMB: mounted, so it counts.)
        None => true,
    })
}

/// The nearest folder at or above `p` that exists ("/" at worst).
fn nearest(p: &str) -> String {
    if remote_present(p) == Some(true) {
        return p.to_string();
    }
    let mut p = Path::new(p);
    while !p.is_dir() {
        match p.parent() {
            Some(up) => p = up,
            None => return "/".into(),
        }
    }
    p.to_string_lossy().into_owned()
}

/// What's left of `s` that can still be opened.
pub fn clean(s: Session, home: &Path) -> Session {
    let windows = s
        .windows
        .into_iter()
        .filter_map(|w| {
            let tabs: Vec<Tab> = w
                .tabs
                .into_iter()
                .map(|t| clean_tab(t, home, true))
                .collect();
            if tabs.is_empty() {
                return None;
            }
            let current = w.current.min(tabs.len() - 1);
            Some(Window { tabs, current })
        })
        .collect();
    Session { windows }
}

/// Places that aren't folders on disk: the trash, Start, Recent, the
/// Storage page, a tag's files.
fn special(p: &str) -> bool {
    [
        "trash:",
        "start:",
        "recent:",
        "drives:",
        "cleanup:",
        "containers:",
    ]
    .contains(&p)
        || p.starts_with("tag:")
}

fn clean_tab(t: Tab, home: &Path, outer: bool) -> Tab {
    let ok = |p: &String| {
        p.starts_with('/') && remote_present(p).unwrap_or_else(|| Path::new(p).is_dir())
    };
    let path = if special(&t.path) {
        t.path
    } else if t.path.starts_with('/') {
        nearest(&t.path)
    } else {
        home.to_string_lossy().into_owned()
    };
    let mut back: Vec<String> = t.back.into_iter().filter(|p| ok(p) || special(p)).collect();
    back.drain(..back.len().saturating_sub(HISTORY));
    let forward = t
        .forward
        .into_iter()
        .filter(|p| ok(p) || special(p))
        .take(HISTORY)
        .collect();
    // Only a tab splits, once; its other side is a plain one.
    let second = if outer {
        t.second.map(|b| Box::new(clean_tab(*b, home, false)))
    } else {
        None
    };
    let active = if second.is_some() { t.active.min(1) } else { 0 };
    Tab {
        path,
        back,
        forward,
        vertical: t.vertical && second.is_some(),
        active,
        second,
    }
}

pub fn load_from(dir: &Path, home: &Path) -> Session {
    let s = std::fs::read_to_string(dir.join("session.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default();
    clean(s, home)
}

/// Written beside and renamed over, so a crash never leaves half a file.
pub fn save_to(dir: &Path, s: &Session) -> Result<(), String> {
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = dir.join("session.json.tmp");
    let text = serde_json::to_string(s).map_err(|e| e.to_string())?;
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, dir.join("session.json")).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    fn scratch(name: &str) -> Scratch {
        let d = Scratch::new(&format!("files-session-{name}"));
        d
    }

    #[test]
    fn round_trip() {
        let d = scratch("rt");
        let a = d.join("a").to_string_lossy().into_owned();
        std::fs::create_dir_all(&a).unwrap();
        let s = Session {
            windows: vec![Window {
                tabs: vec![Tab {
                    path: a.clone(),
                    back: vec![d.to_string_lossy().into_owned()],
                    ..Tab::default()
                }],
                current: 0,
            }],
        };
        save_to(&d.join("state"), &s).unwrap();
        assert_eq!(load_from(&d.join("state"), &d), s);
        assert!(!d.join("state/session.json.tmp").exists());
    }

    #[test]
    fn gone_folders_fall_back() {
        let d = scratch("gone");
        let kept = d.join("kept").to_string_lossy().into_owned();
        std::fs::create_dir_all(&kept).unwrap();
        let gone = d.join("kept/deleted/deeper").to_string_lossy().into_owned();
        let s = Session {
            windows: vec![
                Window {
                    tabs: vec![Tab {
                        path: gone.clone(),
                        back: vec![gone.clone(), kept.clone()],
                        forward: vec![gone, "relative".into()],
                        ..Tab::default()
                    }],
                    current: 7,
                },
                Window::default(),
            ],
        };
        let c = clean(s, &d);
        assert_eq!(c.windows.len(), 1, "a window with no tabs is dropped");
        let w = &c.windows[0];
        assert_eq!(w.current, 0);
        assert_eq!(w.tabs[0].path, kept, "the nearest folder still there");
        assert_eq!(w.tabs[0].back, vec![kept]);
        assert!(w.tabs[0].forward.is_empty());
    }

    #[test]
    fn missing_or_broken_file_is_empty() {
        let d = scratch("broken");
        assert_eq!(load_from(&d, &d), Session::default());
        std::fs::write(d.join("session.json"), "{not json").unwrap();
        assert_eq!(load_from(&d, &d), Session::default());
    }

    #[test]
    fn history_is_capped() {
        let d = scratch("cap");
        let p = d.to_string_lossy().into_owned();
        let s = Session {
            windows: vec![Window {
                tabs: vec![Tab {
                    path: p.clone(),
                    back: vec![p.clone(); 80],
                    forward: vec![p.clone(); 80],
                    ..Tab::default()
                }],
                current: 0,
            }],
        };
        let c = clean(s, &d);
        assert_eq!(c.windows[0].tabs[0].back.len(), HISTORY);
        assert_eq!(c.windows[0].tabs[0].forward.len(), HISTORY);
    }

    #[test]
    fn split_tabs() {
        let d = scratch("split");
        let p = d.to_string_lossy().into_owned();
        let gone = d.join("gone").to_string_lossy().into_owned();
        let inner = Tab {
            path: gone,
            second: Some(Box::new(Tab::default())),
            active: 1,
            ..Tab::default()
        };
        let s = Session {
            windows: vec![Window {
                tabs: vec![
                    Tab {
                        path: p.clone(),
                        second: Some(Box::new(inner)),
                        vertical: true,
                        active: 5,
                        ..Tab::default()
                    },
                    Tab {
                        path: "trash:".into(),
                        vertical: true,
                        active: 1,
                        ..Tab::default()
                    },
                ],
                current: 0,
            }],
        };
        let c = clean(s, &d);
        let t = &c.windows[0].tabs[0];
        let second = t.second.as_ref().unwrap();
        assert_eq!(second.path, p, "the other side's folder is checked too");
        assert!(second.second.is_none(), "a side doesn't split again");
        assert_eq!((t.vertical, t.active), (true, 1));
        let plain = &c.windows[0].tabs[1];
        assert_eq!(plain.path, "trash:", "the trash stays the trash");
        assert_eq!(
            (plain.vertical, plain.active),
            (false, 0),
            "unsplit: no side to be in"
        );
    }
}
