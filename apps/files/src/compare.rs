//! Comparing two folders, and syncing them.
//!
//! `compare` walks both trees and lists what differs: only on one side,
//! newer on one side, or different with nothing to tell which is newer.
//! Files of the same size and modification time (within 2 seconds, for FAT
//! drives) are the same, or, with `content`, files of the same size are
//! read and compared byte for byte.
//!
//! `plan` turns that into what a sync would do:
//! - **update** (one way): new and newer files from the source go to the
//!   target; nothing is removed.
//! - **mirror** (one way): the target ends up exactly like the source:
//!   anything different is replaced, and what's only in the target goes to
//!   the Trash (nothing is deleted for good).
//! - **two-way**: each side gets what's new or newer on the other; files
//!   that differ with no way to tell which is newer are left alone and
//!   listed.

use std::{
    fs,
    io::Read,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    OnlyLeft,
    OnlyRight,
    LeftNewer,
    RightNewer,
    /// Different, with the same time (or a file on one side, a folder on
    /// the other): nothing says which to keep.
    Differs,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Side {
    pub size: u64,
    pub modified: i64,
    pub dir: bool,
    /// A folder: how many files are in it.
    pub files: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Entry {
    /// Relative to both roots ("photos/2026/a.jpg").
    pub rel: String,
    pub state: State,
    pub left: Option<Side>,
    pub right: Option<Side>,
}

/// Times this close are the same (FAT keeps 2-second times).
const SLACK: i64 = 2;

fn side(m: &fs::Metadata) -> Side {
    Side {
        size: m.len(),
        modified: m.mtime(),
        dir: m.is_dir(),
        files: 0,
    }
}

/// A folder's file count and total size (only on one side: shown whole).
fn whole(p: &Path, s: &mut Side, stop: &AtomicBool) {
    let Ok(rd) = fs::read_dir(p) else { return };
    for e in rd.flatten() {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        let Ok(m) = e.path().symlink_metadata() else {
            continue;
        };
        if m.is_dir() {
            whole(&e.path(), s, stop);
        } else {
            s.files += 1;
            s.size += m.len();
        }
    }
}

fn same_content(a: &Path, b: &Path, stop: &AtomicBool) -> bool {
    let (Ok(mut x), Ok(mut y)) = (fs::File::open(a), fs::File::open(b)) else {
        return false;
    };
    let (mut bx, mut by) = (vec![0u8; 1 << 16], vec![0u8; 1 << 16]);
    loop {
        if stop.load(Ordering::Relaxed) {
            return true; // stopped: no claim either way
        }
        let n = match x.read(&mut bx) {
            Ok(n) => n,
            Err(_) => return false,
        };
        if y.read_exact(&mut by[..n]).is_err() {
            return false;
        }
        if n == 0 {
            // Both done only if the other has nothing more either.
            return y.read(&mut by[..1]).map(|m| m == 0).unwrap_or(false);
        }
        if bx[..n] != by[..n] {
            return false;
        }
    }
}

fn names(p: &Path) -> Vec<String> {
    let mut v: Vec<String> = fs::read_dir(p)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    v.sort();
    v
}

/// What differs between `left` and `right` (sorted by path). `tell` hears
/// how many items have been looked at so far.
pub fn compare(
    left: &Path,
    right: &Path,
    content: bool,
    stop: &AtomicBool,
    tell: &mut dyn FnMut(u64),
) -> Vec<Entry> {
    let mut out = vec![];
    let mut seen = 0u64;
    walk(left, right, "", content, stop, tell, &mut seen, &mut out);
    out
}

#[allow(clippy::too_many_arguments)]
fn walk(
    l: &Path,
    r: &Path,
    rel: &str,
    content: bool,
    stop: &AtomicBool,
    tell: &mut dyn FnMut(u64),
    seen: &mut u64,
    out: &mut Vec<Entry>,
) {
    let (ln, rn) = (names(l), names(r));
    let mut all: Vec<&String> = ln.iter().chain(rn.iter()).collect();
    all.sort();
    all.dedup();
    for n in all {
        if stop.load(Ordering::Relaxed) {
            return;
        }
        *seen += 1;
        tell(*seen);
        let (lp, rp) = (l.join(n), r.join(n));
        let path = if rel.is_empty() {
            n.clone()
        } else {
            format!("{rel}/{n}")
        };
        let (lm, rm) = (lp.symlink_metadata().ok(), rp.symlink_metadata().ok());
        let one = |p: &Path, m: &fs::Metadata| {
            let mut s = side(m);
            if s.dir {
                s.size = 0;
                whole(p, &mut s, stop);
            }
            s
        };
        match (lm, rm) {
            (Some(a), None) => out.push(Entry {
                rel: path,
                state: State::OnlyLeft,
                left: Some(one(&lp, &a)),
                right: None,
            }),
            (None, Some(b)) => out.push(Entry {
                rel: path,
                state: State::OnlyRight,
                left: None,
                right: Some(one(&rp, &b)),
            }),
            (Some(a), Some(b)) if a.is_dir() && b.is_dir() => {
                walk(&lp, &rp, &path, content, stop, tell, seen, out)
            }
            (Some(a), Some(b)) => {
                let (sa, sb) = (side(&a), side(&b));
                let state = if sa.dir != sb.dir {
                    Some(State::Differs)
                } else if sa.size == sb.size
                    && (content || (sa.modified - sb.modified).abs() <= SLACK)
                {
                    // Same size: the same if the times match, or (content)
                    // if the bytes do.
                    if !content || same_content(&lp, &rp, stop) {
                        None
                    } else {
                        Some(by_time(&sa, &sb))
                    }
                } else {
                    Some(by_time(&sa, &sb))
                };
                if let Some(state) = state {
                    out.push(Entry {
                        rel: path,
                        state,
                        left: Some(sa),
                        right: Some(sb),
                    });
                }
            }
            (None, None) => {}
        }
    }
}

fn by_time(a: &Side, b: &Side) -> State {
    if a.modified - b.modified > SLACK {
        State::LeftNewer
    } else if b.modified - a.modified > SLACK {
        State::RightNewer
    } else {
        State::Differs
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Plan {
    pub left: PathBuf,
    pub right: PathBuf,
    /// Relative paths copied left → right (replacing what's there).
    pub to_right: Vec<String>,
    /// Relative paths copied right → left.
    pub to_left: Vec<String>,
    /// Relative paths in the right folder moved to the Trash.
    pub trash_right: Vec<String>,
    pub trash_left: Vec<String>,
    /// Left alone: different, and nothing says which to keep.
    pub unsure: Vec<String>,
}

/// What syncing would do. `mode`: update | mirror | two-way; `towards`:
/// "right" or "left" for the one-way modes.
pub fn plan(left: &Path, right: &Path, entries: &[Entry], mode: &str, towards: &str) -> Plan {
    let mut p = Plan {
        left: left.to_path_buf(),
        right: right.to_path_buf(),
        ..Default::default()
    };
    // One way: work as if towards the right, then swap if it's the left.
    let flip = towards == "left" && mode != "two-way";
    for e in entries {
        let s = if flip {
            match e.state {
                State::OnlyLeft => State::OnlyRight,
                State::OnlyRight => State::OnlyLeft,
                State::LeftNewer => State::RightNewer,
                State::RightNewer => State::LeftNewer,
                State::Differs => State::Differs,
            }
        } else {
            e.state
        };
        let r = e.rel.clone();
        match (mode, s) {
            (_, State::OnlyLeft) | (_, State::LeftNewer) => p.to_right.push(r),
            ("mirror", State::RightNewer) | ("mirror", State::Differs) => p.to_right.push(r),
            ("mirror", State::OnlyRight) => p.trash_right.push(r),
            ("two-way", State::OnlyRight) | ("two-way", State::RightNewer) => p.to_left.push(r),
            (_, State::Differs) | ("update", State::RightNewer) => p.unsure.push(r),
            ("update", State::OnlyRight) => {}
            _ => {}
        }
    }
    if flip {
        std::mem::swap(&mut p.to_right, &mut p.to_left);
        std::mem::swap(&mut p.trash_right, &mut p.trash_left);
    }
    p
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, SystemTime};

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("wr-compare-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn write(p: &Path, body: &str, age_secs: u64) {
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, body).unwrap();
        let t = SystemTime::now() - Duration::from_secs(age_secs);
        fs::File::options()
            .write(true)
            .open(p)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }

    fn states(e: &[Entry]) -> Vec<(String, State)> {
        e.iter().map(|x| (x.rel.clone(), x.state)).collect()
    }

    fn sample(n: &str) -> (PathBuf, PathBuf, PathBuf) {
        let d = tmp(n);
        let (l, r) = (d.join("l"), d.join("r"));
        write(&l.join("same.txt"), "same", 100);
        write(&r.join("same.txt"), "same", 100);
        write(&l.join("only-left.txt"), "a", 100);
        write(&r.join("only-right.txt"), "b", 100);
        write(&l.join("docs/newer-left.txt"), "new!", 10);
        write(&r.join("docs/newer-left.txt"), "old", 1000);
        write(&l.join("docs/newer-right.txt"), "old", 1000);
        write(&r.join("docs/newer-right.txt"), "new!", 10);
        write(&l.join("clash.txt"), "one", 500);
        write(&r.join("clash.txt"), "two!", 500);
        write(&l.join("folder-left/a/1.txt"), "1", 100);
        write(&l.join("folder-left/2.txt"), "22", 100);
        (d, l, r)
    }

    #[test]
    fn lists_what_differs() {
        let (d, l, r) = sample("list");
        let stop = AtomicBool::new(false);
        let e = compare(&l, &r, false, &stop, &mut |_| {});
        assert_eq!(
            states(&e),
            vec![
                ("clash.txt".into(), State::Differs),
                ("docs/newer-left.txt".into(), State::LeftNewer),
                ("docs/newer-right.txt".into(), State::RightNewer),
                ("folder-left".into(), State::OnlyLeft),
                ("only-left.txt".into(), State::OnlyLeft),
                ("only-right.txt".into(), State::OnlyRight),
            ]
        );
        // A folder on one side: shown whole, with what's in it.
        let f = e
            .iter()
            .find(|x| x.rel == "folder-left")
            .unwrap()
            .left
            .clone()
            .unwrap();
        assert!(f.dir && f.files == 2 && f.size == 3, "{f:?}");
        fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn content_check_finds_same_size_changes() {
        let d = tmp("content");
        let (l, r) = (d.join("l"), d.join("r"));
        // Same size and time, different bytes: only a content check sees it.
        write(&l.join("a.bin"), "abcd", 100);
        write(&r.join("a.bin"), "abce", 100);
        // Same bytes, different times: content says they're the same.
        write(&l.join("b.bin"), "xyz", 100);
        write(&r.join("b.bin"), "xyz", 5000);
        let stop = AtomicBool::new(false);
        assert_eq!(
            states(&compare(&l, &r, false, &stop, &mut |_| {})),
            vec![("b.bin".into(), State::LeftNewer)]
        );
        assert_eq!(
            states(&compare(&l, &r, true, &stop, &mut |_| {})),
            vec![("a.bin".into(), State::Differs)]
        );
        fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn plans_for_each_mode() {
        let (d, l, r) = sample("plan");
        let stop = AtomicBool::new(false);
        let e = compare(&l, &r, false, &stop, &mut |_| {});
        let up = plan(&l, &r, &e, "update", "right");
        assert_eq!(
            up.to_right,
            ["docs/newer-left.txt", "folder-left", "only-left.txt"]
        );
        assert!(up.to_left.is_empty() && up.trash_right.is_empty());
        assert_eq!(up.unsure, ["clash.txt", "docs/newer-right.txt"]);

        let mirror = plan(&l, &r, &e, "mirror", "right");
        assert_eq!(
            mirror.to_right,
            [
                "clash.txt",
                "docs/newer-left.txt",
                "docs/newer-right.txt",
                "folder-left",
                "only-left.txt"
            ]
        );
        assert_eq!(mirror.trash_right, ["only-right.txt"]);
        assert!(mirror.unsure.is_empty());

        let two = plan(&l, &r, &e, "two-way", "right");
        assert_eq!(
            two.to_right,
            ["docs/newer-left.txt", "folder-left", "only-left.txt"]
        );
        assert_eq!(two.to_left, ["docs/newer-right.txt", "only-right.txt"]);
        assert_eq!(two.unsure, ["clash.txt"]);

        // Towards the left: the same, the other way round.
        let back = plan(&l, &r, &e, "mirror", "left");
        assert_eq!(
            back.to_left,
            [
                "clash.txt",
                "docs/newer-left.txt",
                "docs/newer-right.txt",
                "only-right.txt"
            ]
        );
        assert_eq!(back.trash_left, ["folder-left", "only-left.txt"]);
        fs::remove_dir_all(d).unwrap();
    }
}
