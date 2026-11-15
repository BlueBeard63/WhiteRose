//! `whiterose-files --index`: makes the search index in a process of its
//! own, at the lowest CPU and disk priority, so it carries on when Files
//! closes and whiterosed can start it with Files closed. One at a time (a
//! lock); a second exits at once.
//!
//! While it runs, `index.progress` beside the index says how far it's got
//! ({pid, started, count, expected}: `expected` is how many entries the
//! index before had, 0 the first time); Files reads it to show a bar and
//! the time left. It's removed when done.
//!
//! What to index and whether to seal it come from your settings (through
//! whiterosed): without the daemon nothing is made, as an index meant to
//! be sealed must never be written in the clear.

use std::{
    fs,
    os::unix::io::AsRawFd,
    path::{Path, PathBuf},
    sync::atomic::AtomicBool,
    time::{Duration, Instant},
};

use serde_json::{json, Value};

use crate::{finder, search};

/// How often the progress file is rewritten.
const EVERY: Duration = Duration::from_millis(400);

#[derive(serde::Deserialize, serde::Serialize, Debug, Default, Clone, Copy, PartialEq)]
pub struct Progress {
    pub pid: u32,
    /// Unix ms.
    pub started: f64,
    pub count: u64,
    pub expected: u64,
}

pub fn progress_file(index: &Path) -> PathBuf {
    index.with_extension("progress")
}

fn lock_file(index: &Path) -> PathBuf {
    index.with_extension("lock")
}

pub fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as f64)
        .unwrap_or(0.0)
}

/// The index being made right now, if one is: its progress file, while the
/// process that wrote it is still alive.
pub fn running(index: &Path) -> Option<Progress> {
    let p: Progress = serde_json::from_slice(&fs::read(progress_file(index)).ok()?).ok()?;
    // SAFETY: signal 0 only asks whether the process exists.
    let alive = p.pid != 0 && unsafe { libc::kill(p.pid as libc::pid_t, 0) } == 0;
    alive.then_some(p)
}

/// The share done (0–1) and seconds left, from `p` at `now` (unix ms).
/// None while there's nothing to go on (the first index, or just started).
pub fn estimate(p: &Progress, now: f64) -> Option<(f64, f64)> {
    if p.expected == 0 || p.count == 0 {
        return None;
    }
    // (It can find more than last time: then it's nearly done, not over.)
    let done = (p.count as f64 / p.expected as f64).min(0.99);
    let took = (now - p.started).max(0.0) / 1000.0;
    if took < 1.0 {
        return None;
    }
    let left = took / done * (1.0 - done);
    Some((done, left))
}

fn setting(key: &str) -> Result<Value, String> {
    whiterose_kit::daemon::request(json!({"op": "get", "key": key}), Duration::from_secs(5))
}

fn write_progress(file: &Path, p: &Progress) {
    let tmp = file.with_extension("progress.tmp");
    if fs::write(&tmp, serde_json::to_vec(p).unwrap_or_default()).is_ok() {
        let _ = fs::rename(&tmp, file);
    }
}

/// Runs `--index`: the exit code.
pub fn main() -> i32 {
    finder::background();
    let index = search::index_file();
    if let Some(d) = index.parent() {
        let _ = fs::create_dir_all(d);
    }
    // One at a time: held until this process ends.
    let Ok(lock) = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(lock_file(&index))
    else {
        eprintln!("whiterose-files: can't open the index lock");
        return 1;
    };
    // SAFETY: flock on a file we hold open.
    if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return 0;
    }

    let (drives, exclude, encrypt) = match (
        setting("files.indexDrives"),
        setting("files.indexExclude"),
        setting("files.encrypt"),
    ) {
        (Ok(d), Ok(x), Ok(e)) => (
            d.as_bool().unwrap_or(true),
            serde_json::from_value::<Vec<String>>(x).unwrap_or_default(),
            e.as_bool().unwrap_or(false),
        ),
        _ => {
            eprintln!("whiterose-files: whiterosed isn't running, so the index wasn't made");
            return 1;
        }
    };
    crate::secure::set_on(encrypt);
    let home = crate::places::home();
    let exclude: Vec<PathBuf> = exclude
        .iter()
        .map(|p| match p.strip_prefix("~/") {
            Some(rest) => home.join(rest),
            None => PathBuf::from(p),
        })
        .collect();

    let expected = search::Index::open(&index).map_or(0, |ix| ix.entries()) as u64;
    let pfile = progress_file(&index);
    let mut p = Progress {
        pid: std::process::id(),
        started: now_ms(),
        count: 0,
        expected,
    };
    write_progress(&pfile, &p);
    let mut last = Instant::now();
    let mut tagged = vec![];
    let stop = AtomicBool::new(false);
    let made = search::build_tagged(
        &finder::roots(drives),
        &exclude,
        &index,
        &stop,
        &mut tagged,
        &mut |n| {
            if last.elapsed() >= EVERY {
                last = Instant::now();
                p.count = n as u64;
                write_progress(&pfile, &p);
            }
        },
    );
    let _ = fs::remove_file(&pfile);
    match made {
        Ok(_) => {
            // Tags other apps put on files: on the list too.
            crate::tags::found(tagged);
            0
        }
        Err(e) => {
            eprintln!("whiterose-files: the index wasn't made: {e}");
            1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    #[test]
    fn estimates_from_the_last_index() {
        let p = Progress {
            pid: 1,
            started: 0.0,
            count: 250,
            expected: 1000,
        };
        // A quarter in 10 s: 30 s to go.
        let (done, left) = estimate(&p, 10_000.0).unwrap();
        assert_eq!(done, 0.25);
        assert!((left - 30.0).abs() < 1e-9);
        // More than last time: nearly done, never over.
        let more = Progress { count: 1500, ..p };
        assert_eq!(estimate(&more, 10_000.0).unwrap().0, 0.99);
        // Nothing to go on.
        assert_eq!(estimate(&Progress { expected: 0, ..p }, 10_000.0), None);
        assert_eq!(estimate(&p, 500.0), None);
    }

    #[test]
    fn running_needs_a_live_writer() {
        let d = Scratch::new("indexer");
        let ix = d.join("index");
        assert_eq!(running(&ix), None);
        let mine = Progress {
            pid: std::process::id(),
            started: 1.0,
            count: 5,
            expected: 9,
        };
        write_progress(&progress_file(&ix), &mine);
        assert_eq!(running(&ix), Some(mine));
        // A writer that's gone (killed mid-way) doesn't count.
        write_progress(
            &progress_file(&ix),
            &Progress {
                pid: i32::MAX as u32,
                ..mine
            },
        );
        assert_eq!(running(&ix), None);
    }
}
