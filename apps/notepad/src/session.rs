//! What Notepad remembers between runs, in $XDG_DATA_HOME/whiterose/notepad:
//! the scratch note (scratch.md, saved as you type) and session.json (the
//! files you had open, where you were in each, and recent files).

use std::{fs, path::PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Tab {
    pub path: String,
    #[serde(default)]
    pub cursor: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Session {
    #[serde(default)]
    pub tabs: Vec<Tab>,
    /// Index of the tab in front (0 is the scratch note; files follow).
    #[serde(default)]
    pub current: i64,
    #[serde(default)]
    pub recent: Vec<String>,
}

pub fn dir() -> PathBuf {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share")
        });
    data.join("whiterose/notepad")
}

pub fn load_from(dir: &std::path::Path) -> Session {
    fs::read_to_string(dir.join("session.json"))
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

pub fn save_to(dir: &std::path::Path, s: &Session) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let text = serde_json::to_string_pretty(s).map_err(|e| e.to_string())?;
    let tmp = dir.join("session.json.new");
    fs::write(&tmp, text).map_err(|e| e.to_string())?;
    fs::rename(tmp, dir.join("session.json")).map_err(|e| e.to_string())
}

pub fn scratch_from(dir: &std::path::Path) -> String {
    fs::read_to_string(dir.join("scratch.md")).unwrap_or_default()
}

pub fn save_scratch_to(dir: &std::path::Path, text: &str) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    crate::doc::write(&dir.join("scratch.md"), text, &crate::doc::Eol::Lf).map(|_| ())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    #[test]
    fn session_and_scratch_round_trip() {
        let d = Scratch::new("notepad-session");
        assert_eq!(load_from(&d), Session::default(), "nothing saved yet");
        assert_eq!(scratch_from(&d), "");
        let s = Session {
            tabs: vec![Tab {
                path: "/a.txt".into(),
                cursor: 42,
            }],
            current: 1,
            recent: vec!["/a.txt".into()],
        };
        save_to(&d, &s).unwrap();
        save_scratch_to(&d, "todo: milk\n").unwrap();
        assert_eq!(load_from(&d), s);
        assert_eq!(scratch_from(&d), "todo: milk\n");
        fs::write(d.join("session.json"), "{broken").unwrap();
        assert_eq!(
            load_from(&d),
            Session::default(),
            "a broken file starts fresh"
        );
    }
}
