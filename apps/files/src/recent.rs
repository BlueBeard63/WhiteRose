//! Recent ("recent:" in the sidebar): files apps opened lately (the shared
//! recently-used.xbel every GTK and KDE app writes) and files changed in
//! the last two weeks (from the search index: nothing is walked), newest
//! first. Folders, hidden files and things in hidden folders are left out.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
};

use crate::search;

/// How far back "changed lately" goes.
const DAYS: i64 = 14;
/// How many are shown.
pub const KEEP: usize = 300;

/// Days since 1970-01-01 for a civil date (Howard Hinnant's algorithm).
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// "2026-11-10T09:30:00.123456Z" (UTC, as XBEL writes it) as unix seconds.
fn iso(s: &str) -> Option<i64> {
    let b = s.as_bytes();
    if b.len() < 19 {
        return None;
    }
    let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let days = days_from_civil(n(0..4)?, n(5..7)?, n(8..10)?);
    Some(days * 86_400 + n(11..13)? * 3600 + n(14..16)? * 60 + n(17..19)?)
}

fn unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

/// %XX in a file:// URI.
fn decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let at = tag.find(&format!(" {name}=\""))? + name.len() + 3;
    let end = tag[at..].find('"')?;
    Some(&tag[at..at + end])
}

/// The local files in an XBEL file, with when each was last used (unix s).
pub fn xbel(text: &str) -> Vec<(PathBuf, i64)> {
    let mut out = vec![];
    for chunk in text.split("<bookmark ").skip(1) {
        let tag = &chunk[..chunk.find('>').unwrap_or(chunk.len())];
        let tag = format!(" {tag}");
        let Some(href) = attr(&tag, "href") else {
            continue;
        };
        let Some(path) = unescape(href).strip_prefix("file://").map(decode) else {
            continue;
        };
        let when = ["visited", "modified", "added"]
            .iter()
            .filter_map(|a| attr(&tag, a).and_then(iso))
            .max()
            .unwrap_or(0);
        out.push((PathBuf::from(path), when));
    }
    out
}

fn hidden(p: &Path, home: &Path) -> bool {
    let rel = p.strip_prefix(home).unwrap_or(p);
    rel.components()
        .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
}

/// Recent files, newest first: those in `xbel_text`, and those the index
/// at `index` says changed since `now - 14 days`.
pub fn files(xbel_text: &str, index: &Path, home: &Path, now: i64) -> Vec<(PathBuf, i64)> {
    let mut best: HashMap<PathBuf, i64> = HashMap::new();
    for (p, t) in xbel(xbel_text) {
        best.entry(p).and_modify(|x| *x = (*x).max(t)).or_insert(t);
    }
    if let Some(ix) = search::Index::open(index) {
        let q = search::Query::default();
        let f = search::Filters {
            kind: String::new(),
            size: (0, u64::MAX),
            since: now - DAYS * 86_400,
        };
        let (hits, _) = ix.find(
            &q,
            &f,
            &|_| true,
            &|_| String::new(),
            &AtomicBool::new(false),
            KEEP * 3,
        );
        for h in hits.into_iter().filter(|h| !h.dir) {
            let t = h.modified / 1000;
            best.entry(PathBuf::from(h.path))
                .and_modify(|x| *x = (*x).max(t))
                .or_insert(t);
        }
    }
    let mut v: Vec<(PathBuf, i64)> = best
        .into_iter()
        .filter(|(p, _)| {
            !hidden(p, home) && std::fs::metadata(p).map(|m| m.is_file()).unwrap_or(false)
        })
        .collect();
    v.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    v.truncate(KEEP);
    v
}

/// Where the shared list is.
pub fn xbel_file() -> PathBuf {
    let data = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::places::home().join(".local/share"));
    data.join("recently-used.xbel")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_xbel() {
        assert_eq!(iso("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(iso("2026-11-10T09:30:05.123Z"), Some(1_794_303_005));
        assert_eq!(iso("nonsense"), None);
        let x = r#"<?xml version="1.0"?>
<xbel version="1.0">
  <bookmark href="file:///home/me/My%20Notes/trip.md" added="2026-11-01T10:00:00Z" modified="2026-11-02T10:00:00Z" visited="2026-11-09T08:00:00Z">
  </bookmark>
  <bookmark href="https://example.com/" added="2026-11-01T10:00:00Z"></bookmark>
  <bookmark href="file:///tmp/a&amp;b.txt" added="2026-11-03T10:00:00Z" modified="2026-11-03T10:00:00Z"/>
</xbel>"#;
        let v = xbel(x);
        assert_eq!(v.len(), 2);
        assert_eq!(v[0].0, PathBuf::from("/home/me/My Notes/trip.md"));
        assert_eq!(v[0].1, iso("2026-11-09T08:00:00Z").unwrap());
        assert_eq!(v[1].0, PathBuf::from("/tmp/a&b.txt"));
    }

    #[test]
    fn newest_first_without_hidden_or_gone() {
        let d = std::env::temp_dir().join(format!("wr-recent-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join(".cache")).unwrap();
        for n in ["old.txt", "new.txt", ".cache/x"] {
            std::fs::write(d.join(n), n).unwrap();
        }
        let uri = |n: &str| format!("file://{}", d.join(n).display());
        let x = format!(
            r#"<bookmark href="{}" visited="2026-01-01T00:00:00Z"/><bookmark href="{}" visited="2026-06-01T00:00:00Z"/><bookmark href="{}" visited="2026-07-01T00:00:00Z"/><bookmark href="{}" visited="2026-08-01T00:00:00Z"/>"#,
            uri("old.txt"),
            uri("new.txt"),
            uri(".cache/x"),
            uri("gone.txt")
        );
        // No index: only what's in the list.
        let v = files(&x, &d.join("no-index"), &d, 0);
        let names: Vec<_> = v
            .iter()
            .map(|(p, _)| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, vec!["new.txt", "old.txt"]);
        std::fs::remove_dir_all(&d).unwrap();
    }
}
