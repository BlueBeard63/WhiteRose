//! The open windows and their tabs, kept between runs in
//! $XDG_STATE_HOME/whiterose/files/session.json. On the way back in, tabs
//! whose folder has gone open the nearest folder that's still there, and
//! history entries that have gone are dropped.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// History kept per tab, each way.
const HISTORY: usize = 50;

/// A tab: its folder and history. (How a folder is shown is the folder's
/// own business: views.rs.)
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
#[serde(default)]
pub struct Tab {
    pub path: String,
    pub back: Vec<String>,
    pub forward: Vec<String>,
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

/// The nearest folder at or above `p` that exists ("/" at worst).
fn nearest(p: &str) -> String {
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
    let ok = |p: &String| p.starts_with('/') && Path::new(p).is_dir();
    let windows = s
        .windows
        .into_iter()
        .filter_map(|w| {
            let tabs: Vec<Tab> = w
                .tabs
                .into_iter()
                .map(|t| {
                    let path = if t.path.starts_with('/') {
                        nearest(&t.path)
                    } else {
                        home.to_string_lossy().into_owned()
                    };
                    let mut back: Vec<String> = t.back.into_iter().filter(ok).collect();
                    back.drain(..back.len().saturating_sub(HISTORY));
                    let forward = t.forward.into_iter().filter(ok).take(HISTORY).collect();
                    Tab {
                        path,
                        back,
                        forward,
                    }
                })
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

    fn scratch(name: &str) -> PathBuf {
        let d =
            std::env::temp_dir().join(format!("wr-files-session-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
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
                }],
                current: 0,
            }],
        };
        let c = clean(s, &d);
        assert_eq!(c.windows[0].tabs[0].back.len(), HISTORY);
        assert_eq!(c.windows[0].tabs[0].forward.len(), HISTORY);
    }
}
