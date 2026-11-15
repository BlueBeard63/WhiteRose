//! Exporting Files' settings to one file, and bringing them back (on this
//! computer or another): every `files.*` setting from whiterosed, folder
//! views, workspaces, folder colours and icons, saved servers (never their
//! passwords: those stay in the keyring) and bookmarks.
//!
//! Importing replaces what's there; what was there is saved first
//! (before-import.json beside the rest) so it can be put back.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use serde_json::{json, Value};

/// Values by name (settings by key, lists by part).
pub type Named = BTreeMap<String, Value>;

pub const FORMAT: &str = "whiterose-files-settings";
const VERSION: u64 = 1;

/// The files that are part of it: (name in the export, where it is).
pub fn parts() -> Vec<(&'static str, PathBuf)> {
    let state = crate::session::dir();
    vec![
        ("views", state.join("views.json")),
        ("workspaces", state.join("workspaces.json")),
        ("looks", state.join("looks.json")),
        ("servers", crate::shares::servers_file()),
        (
            "bookmarks",
            crate::places::config_dir().join("gtk-3.0/bookmarks"),
        ),
    ]
}

/// Everything in one value. `settings` is every setting whiterosed has
/// (only files.* are kept); a part that isn't there is left out.
pub fn export(settings: &Value, parts: &[(&str, PathBuf)]) -> Value {
    let s: BTreeMap<String, Value> = settings
        .as_object()
        .map(|o| {
            o.iter()
                .filter(|(k, _)| k.starts_with("files."))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default();
    let mut files = serde_json::Map::new();
    for (name, path) in parts {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        // JSON stays JSON (readable in the export); bookmarks are text.
        let v = serde_json::from_str::<Value>(&text).unwrap_or(Value::String(text));
        files.insert(name.to_string(), v);
    }
    json!({ "format": FORMAT, "version": VERSION, "settings": s, "files": files })
}

/// What an import would bring in: (settings, files) after checking it's
/// one of these, and no newer than this Files understands.
pub fn read(v: &Value) -> Result<(Named, Named), String> {
    if v.get("format").and_then(Value::as_str) != Some(FORMAT) {
        return Err("That isn't a Files settings export".into());
    }
    if v.get("version").and_then(Value::as_u64).unwrap_or(0) > VERSION {
        return Err("It was exported by a newer Files: update this one first".into());
    }
    let settings = v
        .get("settings")
        .and_then(Value::as_object)
        .map(|o| {
            o.iter()
                .filter(|(k, _)| k.starts_with("files."))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default();
    let names: Vec<&str> = parts().iter().map(|(n, _)| *n).collect();
    let files = v
        .get("files")
        .and_then(Value::as_object)
        .map(|o| {
            o.iter()
                .filter(|(k, _)| names.contains(&k.as_str()))
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect()
        })
        .unwrap_or_default();
    Ok((settings, files))
}

/// Writes the files of an import (each replaced whole, safely).
pub fn write_files(
    files: &BTreeMap<String, Value>,
    parts: &[(&str, PathBuf)],
) -> Result<usize, String> {
    let mut n = 0;
    for (name, path) in parts {
        let Some(v) = files.get(*name) else { continue };
        let text = match v {
            Value::String(s) => s.clone(),
            other => serde_json::to_string_pretty(other).map_err(|e| e.to_string())?,
        };
        write(path, &text)?;
        n += 1;
    }
    Ok(n)
}

fn write(path: &Path, text: &str) -> Result<(), String> {
    if let Some(d) = path.parent() {
        std::fs::create_dir_all(d).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("import-tmp");
    std::fs::write(&tmp, text).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

/// Where what was there before an import is kept.
pub fn backup_file() -> PathBuf {
    crate::session::dir().join("before-import.json")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    #[test]
    fn round_trip() {
        let d = Scratch::new("prefs");
        let parts: Vec<(&str, PathBuf)> = vec![
            ("views", d.join("views.json")),
            ("bookmarks", d.join("bookmarks")),
            ("looks", d.join("looks.json")),
        ];
        std::fs::write(d.join("views.json"), r#"{"/home/x": {"mode": "grid"}}"#).unwrap();
        std::fs::write(d.join("bookmarks"), "file:///home/x/Code Code\n").unwrap();
        let settings = json!({"files.hidden": true, "files.keys": {"rename": "F2"}, "bar.screens": "main", "calendar.sources": [{"secret": "x"}]});
        let e = export(&settings, &parts);
        // Only Files' settings; looks.json wasn't there.
        assert_eq!(
            e["settings"]
                .as_object()
                .unwrap()
                .keys()
                .collect::<Vec<_>>(),
            vec!["files.hidden", "files.keys"]
        );
        assert_eq!(e["files"]["views"]["/home/x"]["mode"], "grid");
        assert_eq!(e["files"]["bookmarks"], "file:///home/x/Code Code\n");
        assert!(e["files"].get("looks").is_none());

        // Brought back elsewhere.
        let other = d.join("other");
        let parts2: Vec<(&str, PathBuf)> = vec![
            ("views", other.join("views.json")),
            ("bookmarks", other.join("gtk/bookmarks")),
            ("looks", other.join("looks.json")),
        ];
        let text = serde_json::to_string(&e).unwrap();
        let (s, f) = read(&serde_json::from_str(&text).unwrap()).unwrap();
        assert_eq!(s.len(), 2);
        assert_eq!(write_files(&f, &parts2).unwrap(), 2);
        assert_eq!(
            std::fs::read_to_string(other.join("gtk/bookmarks")).unwrap(),
            "file:///home/x/Code Code\n"
        );
        let v: Value =
            serde_json::from_str(&std::fs::read_to_string(other.join("views.json")).unwrap())
                .unwrap();
        assert_eq!(v["/home/x"]["mode"], "grid");
    }

    #[test]
    fn refuses_what_it_isnt() {
        assert!(read(&json!({"a": 1})).is_err());
        assert!(read(&json!({"format": FORMAT, "version": 99}))
            .unwrap_err()
            .contains("newer"));
        // Other apps' settings and unknown files are ignored.
        let (s, f) = read(&json!({"format": FORMAT, "version": 1, "settings": {"bar.screens": "x", "files.zoom": 1.2}, "files": {"../../etc/passwd": "x", "views": {}}})).unwrap();
        assert_eq!(s.keys().collect::<Vec<_>>(), vec!["files.zoom"]);
        assert_eq!(f.keys().collect::<Vec<_>>(), vec!["views"]);
    }
}
