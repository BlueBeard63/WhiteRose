//! Jumping to a folder by a few letters of it, from anywhere (type in the
//! address bar without a leading / or ~): "proj wr" finds
//! ~/Projects/Personal/WhiteRose. Folders you go to often and lately rank
//! first (remembered in visits.json, like zoxide), then folders from the
//! search index whose name fits.

use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

use serde::{Deserialize, Serialize};

use crate::search;

/// Folders remembered (the least used go first past this).
const KEEP: usize = 600;

#[derive(Serialize, Deserialize, Default, Clone, Copy, Debug, PartialEq)]
pub struct Visit {
    pub count: u32,
    /// unix s
    pub last: i64,
}

pub fn file() -> PathBuf {
    crate::session::dir().join("visits.json")
}

pub fn load(file: &Path) -> HashMap<String, Visit> {
    fs::read_to_string(file)
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// How often and how lately, as one number (zoxide's weighting).
pub fn frecency(v: &Visit, now: i64) -> f64 {
    let age = now - v.last;
    let w = match age {
        a if a < 3600 => 4.0,
        a if a < 86_400 => 2.0,
        a if a < 7 * 86_400 => 0.5,
        _ => 0.25,
    };
    v.count as f64 * w
}

/// A folder was opened.
pub fn record(file: &Path, path: &str, now: i64) {
    if !path.starts_with('/') {
        return;
    }
    let mut all = load(file);
    let v = all.entry(path.to_string()).or_default();
    v.count = v.count.saturating_add(1);
    v.last = now;
    if all.len() > KEEP {
        // Forget the least worth remembering.
        let mut by: Vec<(String, f64)> = all
            .iter()
            .map(|(p, v)| (p.clone(), frecency(v, now)))
            .collect();
        by.sort_by(|a, b| a.1.total_cmp(&b.1));
        for (p, _) in by.into_iter().take(all.len() - KEEP) {
            all.remove(&p);
        }
    }
    if let Some(d) = file.parent() {
        let _ = fs::create_dir_all(d);
    }
    let tmp = file.with_extension("json.tmp");
    if fs::write(&tmp, serde_json::to_string(&all).unwrap_or_default()).is_ok() {
        let _ = fs::rename(&tmp, file);
    }
}

/// How well `words` fit `path` (None: they don't). Every word's letters
/// must appear in order in the path; the last word should be in the
/// folder's own name. Runs of letters, word starts, and the name itself
/// count for more.
pub fn fuzzy(words: &[String], path: &str) -> Option<i32> {
    if words.is_empty() {
        return None;
    }
    let lower = path.to_lowercase();
    let name_at = lower.rfind('/').map(|i| i + 1).unwrap_or(0);
    let p: Vec<char> = lower.chars().collect();
    let name_start = lower[..name_at].chars().count();
    let mut at = 0usize;
    let mut score = 0;
    for (wi, w) in words.iter().enumerate() {
        let last = wi == words.len() - 1;
        // The last word: within the folder's own name.
        if last && at < name_start {
            at = name_start;
        }
        let mut prev: Option<usize> = None;
        for c in w.chars() {
            let i = (at..p.len()).find(|&i| p[i] == c)?;
            if prev == Some(i.wrapping_sub(1)) {
                score += 6; // a run
            }
            if i == 0 || !p[i - 1].is_alphanumeric() {
                score += 8; // a word's start
            }
            prev = Some(i);
            at = i + 1;
        }
        if last {
            let name = &lower[name_at..];
            if name == w.as_str() {
                score += 60;
            } else if name.starts_with(w.as_str()) {
                score += 35;
            } else if name.contains(w.as_str()) {
                score += 20;
            }
        }
    }
    Some(score - (p.len() as i32).min(80) / 8)
}

#[derive(Serialize, Debug, PartialEq)]
pub struct Hit {
    pub path: String,
    pub name: String,
}

/// The best folders for `text`, up to `keep`.
pub fn find(
    text: &str,
    visits: &HashMap<String, Visit>,
    index: Option<&search::Index>,
    now: i64,
    keep: usize,
) -> Vec<Hit> {
    let words: Vec<String> = text.split_whitespace().map(|w| w.to_lowercase()).collect();
    let Some(last) = words.last() else {
        return vec![];
    };
    let mut best: HashMap<String, f64> = HashMap::new();
    for (p, v) in visits {
        if let Some(s) = fuzzy(&words, p) {
            best.insert(p.clone(), s as f64 + frecency(v, now) * 4.0);
        }
    }
    if let Some(ix) = index {
        // Folders whose name has the last word in it; then the whole fit.
        let q = search::parse_with(last, false, false);
        let f = search::Filters {
            kind: "folder".into(),
            size: (0, u64::MAX),
            since: i64::MIN,
        };
        let (hits, _) = ix.find(
            &q,
            &f,
            &|_| true,
            &|_| "inode/directory".into(),
            &AtomicBool::new(false),
            400,
        );
        for h in hits {
            if let Some(s) = fuzzy(&words, &h.path) {
                best.entry(h.path).or_insert(s as f64);
            }
        }
    }
    let mut v: Vec<(String, f64)> = best
        .into_iter()
        .filter(|(p, _)| Path::new(p).is_dir())
        .collect();
    v.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.len().cmp(&b.0.len())));
    v.truncate(keep);
    v.into_iter()
        .map(|(path, _)| Hit {
            name: Path::new(&path)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.clone()),
            path,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn w(s: &str) -> Vec<String> {
        s.split_whitespace().map(str::to_string).collect()
    }

    #[test]
    fn fuzzy_matching() {
        let wr = "/home/me/Projects/Personal/WhiteRose";
        assert!(fuzzy(&w("proj wr"), wr).is_some());
        assert!(fuzzy(&w("whiterose"), wr).is_some());
        // The last word must be in the folder's own name.
        assert!(fuzzy(&w("proj"), wr).is_none());
        // Letters out of order don't fit.
        assert!(fuzzy(&w("esorw"), wr).is_none());
        // The name itself beats letters spread over the path.
        let exact = fuzzy(&w("docs"), "/home/me/docs").unwrap();
        let spread = fuzzy(&w("docs"), "/home/me/Dog cats").unwrap();
        assert!(exact > spread, "{exact} {spread}");
        assert!(fuzzy(&[], wr).is_none());
    }

    #[test]
    fn visits_rank_and_are_remembered() {
        let d = std::env::temp_dir().join(format!("wr-jump-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        for p in ["a/notes", "b/notes", "c/nothing"] {
            fs::create_dir_all(d.join(p)).unwrap();
        }
        let f = d.join("visits.json");
        let now = 1_000_000;
        let b = d.join("b/notes").to_string_lossy().into_owned();
        for _ in 0..5 {
            record(&f, &b, now);
        }
        record(&f, &d.join("a/notes").to_string_lossy(), now - 30 * 86_400);
        record(&f, "trash:", now); // not a folder: ignored
        let v = load(&f);
        assert_eq!(v.len(), 2);
        assert_eq!(
            v[&b],
            Visit {
                count: 5,
                last: now
            }
        );
        let hits = find("notes", &v, None, now, 8);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].path, b, "often and lately first");
        assert!(
            frecency(
                &Visit {
                    count: 1,
                    last: now
                },
                now
            ) > frecency(
                &Visit {
                    count: 1,
                    last: now - 30 * 86_400
                },
                now
            )
        );
        fs::remove_dir_all(&d).unwrap();
    }
}
