//! Files' log: what it did (copies, moves, trashing, extracting, mounting)
//! and what went wrong, in $XDG_STATE_HOME/whiterose/files/files.log (only
//! yours). Kept to about a megabyte: the older half goes to files.log.1.
//! Diagnostics (☰ › Help) gathers it with what Files is running on.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::OpenOptionsExt,
    path::{Path, PathBuf},
    sync::Mutex,
};

/// Rolled over past this.
const LIMIT: u64 = 1 << 20;

static LOCK: Mutex<()> = Mutex::new(());

pub fn file() -> PathBuf {
    crate::session::dir().join("files.log")
}

fn stamp() -> String {
    // SAFETY: time can't fail.
    stamp_of(unsafe { libc::time(std::ptr::null_mut()) })
}

/// Unix seconds as local "YYYY-MM-DD hh:mm:ss".
pub fn stamp_of(t: i64) -> String {
    // SAFETY: localtime_r fills `tm` from a valid time_t.
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        format!(
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            tm.tm_year + 1900,
            tm.tm_mon + 1,
            tm.tm_mday,
            tm.tm_hour,
            tm.tm_min,
            tm.tm_sec
        )
    }
}

/// One line (newlines in `msg` become spaces) at `level` ("info", "warn", "error").
pub fn write_to(file: &Path, level: &str, msg: &str) {
    let _held = LOCK.lock();
    if let Some(dir) = file.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if fs::metadata(file).map(|m| m.len() > LIMIT).unwrap_or(false) {
        let _ = fs::rename(file, file.with_extension("log.1"));
    }
    let line = format!(
        "{} {:5} {}\n",
        stamp(),
        level,
        msg.replace(['\n', '\r'], " ")
    );
    if let Ok(mut f) = OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(file)
    {
        let _ = f.write_all(line.as_bytes());
    }
}

pub fn info(msg: &str) {
    write_to(&file(), "info", msg)
}

pub fn warn(msg: &str) {
    write_to(&file(), "warn", msg)
}

pub fn error(msg: &str) {
    write_to(&file(), "error", msg)
}

/// The last `n` lines (from the older file too if need be).
pub fn tail_of(file: &Path, n: usize) -> Vec<String> {
    let mut lines: Vec<String> = fs::read_to_string(file)
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect();
    if lines.len() < n {
        let older: Vec<String> = fs::read_to_string(file.with_extension("log.1"))
            .unwrap_or_default()
            .lines()
            .map(str::to_string)
            .collect();
        let want = n - lines.len();
        let mut v = older[older.len().saturating_sub(want)..].to_vec();
        v.append(&mut lines);
        lines = v;
    }
    let from = lines.len().saturating_sub(n);
    lines[from..].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;

    #[test]
    fn writes_rolls_and_tails() {
        let d = std::env::temp_dir().join(format!("wr-log-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        let f = d.join("files.log");
        write_to(&f, "info", "copied 3 items\nto Documents");
        let t = tail_of(&f, 10);
        assert_eq!(t.len(), 1);
        assert!(
            t[0].ends_with("info  copied 3 items to Documents"),
            "{}",
            t[0]
        );
        assert_eq!(
            fs::metadata(&f).unwrap().mode() & 0o777,
            0o600,
            "only yours"
        );
        // Past the limit: rolled over, and the tail reaches back into it.
        fs::write(&f, "x\n".repeat((LIMIT / 2 + 10) as usize)).unwrap();
        write_to(&f, "warn", "after");
        assert!(f.with_extension("log.1").exists());
        let t = tail_of(&f, 3);
        assert_eq!(t.len(), 3);
        assert!(t[2].ends_with("after") && t[0] == "x");
    }
}
