//! Transfers that didn't finish: each copy or move is written down when it
//! starts and crossed off when it ends (finished, failed or cancelled). If
//! Files is closed, crashes or the computer goes off part way, the next
//! start finds it still written down and offers to carry on.
//!
//! Carrying on runs it again for what's still to do: what a move already
//! moved is gone from where it was, files already copied are the same and
//! skipped without asking (ops.rs), and the half-written file it was on
//! (".name.wrpart") is cleared away first.

use std::{
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Entry {
    /// Unique across runs (time + id).
    pub key: String,
    /// copy | move
    pub kind: String,
    pub sources: Vec<String>,
    pub dest: String,
    /// When it started (seconds since 1970).
    pub started: u64,
}

pub fn file() -> PathBuf {
    crate::session::dir().join("transfers.json")
}

pub fn load(file: &Path) -> Vec<Entry> {
    fs::read_to_string(file)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn save(file: &Path, list: &[Entry]) {
    if let Some(d) = file.parent() {
        let _ = fs::create_dir_all(d);
    }
    if list.is_empty() {
        let _ = fs::remove_file(file);
        return;
    }
    let tmp = file.with_extension("json.tmp");
    if fs::write(&tmp, serde_json::to_string(list).unwrap_or_default()).is_ok() {
        let _ = fs::rename(&tmp, file);
    }
}

static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

pub fn start(file: &Path, e: Entry) {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut l = load(file);
    l.retain(|x| x.key != e.key);
    l.push(e);
    save(file, &l);
}

pub fn finish(file: &Path, key: &str) {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let mut l = load(file);
    l.retain(|x| x.key != key);
    save(file, &l);
}

/// What's left of an interrupted transfer: its sources still there (a
/// move's already-moved ones aren't), if the destination is.
pub fn remaining(e: &Entry) -> Vec<String> {
    if !Path::new(&e.dest).is_dir() {
        return vec![];
    }
    e.sources
        .iter()
        .filter(|s| fs::symlink_metadata(s).is_ok())
        .cloned()
        .collect()
}

/// Clears the half-written ".name.wrpart" files a stopped copy left in
/// what it was making under `dest`.
pub fn clear_parts(e: &Entry) -> usize {
    fn walk(d: &Path, n: &mut usize, depth: u32) {
        if depth > 64 {
            return;
        }
        let Ok(rd) = fs::read_dir(d) else { return };
        for x in rd.flatten() {
            let p = x.path();
            let name = x.file_name().to_string_lossy().into_owned();
            let Ok(t) = x.file_type() else { continue };
            if t.is_dir() {
                walk(&p, n, depth + 1);
            } else if t.is_file()
                && name.starts_with('.')
                && name.ends_with(".wrpart")
                && fs::remove_file(&p).is_ok()
            {
                *n += 1;
            }
        }
    }
    let mut n = 0;
    let dest = Path::new(&e.dest);
    for s in &e.sources {
        let Some(name) = Path::new(s).file_name() else {
            continue;
        };
        let target = dest.join(name);
        let part = dest.join(format!(".{}.wrpart", name.to_string_lossy()));
        if part.is_file() && fs::remove_file(&part).is_ok() {
            n += 1;
        }
        if target.is_dir() {
            walk(&target, &mut n, 0);
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    #[test]
    fn written_down_and_crossed_off() {
        let d = Scratch::new("journal");
        fs::create_dir_all(d.join("dest/photos/2026")).unwrap();
        fs::create_dir_all(d.join("src/photos")).unwrap();
        let f = d.join("transfers.json");
        let e = Entry {
            key: "1-1".into(),
            kind: "move".into(),
            sources: vec![
                d.join("src/photos").to_string_lossy().into(),
                d.join("src/gone.txt").to_string_lossy().into(),
            ],
            dest: d.join("dest").to_string_lossy().into(),
            started: 1,
        };
        start(&f, e.clone());
        start(
            &f,
            Entry {
                key: "1-2".into(),
                ..e.clone()
            },
        );
        assert_eq!(load(&f).len(), 2);
        finish(&f, "1-2");
        assert_eq!(load(&f), vec![e.clone()]);
        // gone.txt was moved already: only photos is left.
        assert_eq!(
            remaining(&e),
            vec![d.join("src/photos").to_string_lossy().into_owned()]
        );
        // The half-written file is cleared; the finished one stays.
        fs::write(d.join("dest/photos/2026/.big.mov.wrpart"), "half").unwrap();
        fs::write(d.join("dest/photos/2026/done.jpg"), "whole").unwrap();
        assert_eq!(clear_parts(&e), 1);
        assert!(d.join("dest/photos/2026/done.jpg").exists());
        finish(&f, "1-1");
        assert!(!f.exists(), "an empty journal goes");
        // No destination any more: nothing to carry on.
        assert!(remaining(&Entry {
            dest: d.join("nope").to_string_lossy().into(),
            ..e
        })
        .is_empty());
    }
}
