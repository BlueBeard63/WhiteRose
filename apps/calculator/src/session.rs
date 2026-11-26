//! Private, bounded calculation history. Each edit reads the latest state
//! under a lock, so two Calculator windows cannot overwrite one another.

use serde::{Deserialize, Serialize};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    os::{fd::AsRawFd, unix::fs::OpenOptionsExt},
    path::{Path, PathBuf},
};

pub const MAX_HISTORY: usize = 100;
pub const MAX_EXPRESSION: usize = 4096;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Angle {
    #[default]
    Deg,
    Rad,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub expression: String,
    pub result: String,
    pub angle: Angle,
}

#[derive(Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Session {
    pub scientific: bool,
    pub angle: Angle,
    pub show_history: bool,
    pub history: Vec<Entry>,
}

pub fn dir() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/share")
        })
        .join("whiterose/calculator")
}

fn valid(entry: &Entry) -> bool {
    !entry.expression.trim().is_empty()
        && entry.expression.len() <= MAX_EXPRESSION
        && entry.result.len() <= 128
        && entry.result.parse::<f64>().is_ok_and(f64::is_finite)
}

pub fn load(dir: &Path) -> Session {
    let mut data = String::new();
    let mut session: Session = fs::File::open(dir.join("session.json"))
        .and_then(|file| file.take(1024 * 1024).read_to_string(&mut data))
        .ok()
        .and_then(|_| serde_json::from_str(&data).ok())
        .unwrap_or_default();
    session.history.retain(valid);
    session.history.truncate(MAX_HISTORY);
    session
}

pub fn change(dir: &Path, edit: impl FnOnce(&mut Session)) -> Result<Session, String> {
    let update = || -> std::io::Result<Session> {
        fs::create_dir_all(dir)?;
        let lock = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(dir.join("session.lock"))?;
        // SAFETY: lock owns a live fd, and is held until the atomic write ends.
        if unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX) } != 0 {
            return Err(std::io::Error::last_os_error());
        }
        let mut state = load(dir);
        edit(&mut state);
        state.history.retain(valid);
        state.history.truncate(MAX_HISTORY);
        let tmp = dir.join("session.json.new");
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW)
            .open(&tmp)?;
        file.write_all(&serde_json::to_vec(&state)?)?;
        fs::rename(tmp, dir.join("session.json"))?;
        Ok(state)
    };
    update().map_err(|e| e.to_string())
}

pub fn record(dir: &Path, entry: Entry) -> Result<Session, String> {
    if !valid(&entry) {
        return Err("Invalid calculation".into());
    }
    change(dir, |state| {
        state
            .history
            .retain(|e| e.expression != entry.expression || e.angle != entry.angle);
        state.history.insert(0, entry);
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;
    use std::os::unix::fs::PermissionsExt;

    fn entry(n: usize) -> Entry {
        Entry {
            expression: format!("{n}+1"),
            result: (n + 1).to_string(),
            angle: Angle::Deg,
        }
    }

    #[test]
    fn history_is_private_bounded_and_repeated_calculations_move_to_top() {
        let dir = Scratch::new("calculator-history");
        for n in 0..MAX_HISTORY + 3 {
            record(&dir, entry(n)).unwrap();
        }
        let state = load(&dir);
        assert_eq!(state.history.len(), MAX_HISTORY);
        assert_eq!(state.history[0], entry(MAX_HISTORY + 2));
        record(&dir, entry(10)).unwrap();
        assert_eq!(load(&dir).history[0], entry(10));
        assert_eq!(load(&dir).history.len(), MAX_HISTORY);
        assert_eq!(
            fs::metadata(dir.join("session.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
        assert!(!dir.join("session.json.new").exists());
    }

    #[test]
    fn preferences_and_angle_units_round_trip_and_clear_is_persistent() {
        let dir = Scratch::new("calculator-preferences");
        record(&dir, entry(1)).unwrap();
        record(
            &dir,
            Entry {
                angle: Angle::Rad,
                ..entry(1)
            },
        )
        .unwrap();
        let state = change(&dir, |s| {
            s.scientific = true;
            s.angle = Angle::Rad;
            s.show_history = true;
        })
        .unwrap();
        assert_eq!(state.history.len(), 2);
        assert_eq!(load(&dir), state);
        change(&dir, |s| s.history.clear()).unwrap();
        assert!(load(&dir).history.is_empty());
        assert!(load(&dir).scientific);
    }

    #[test]
    fn broken_history_and_invalid_entries_do_not_stop_the_app() {
        let dir = Scratch::new("calculator-corrupt");
        fs::write(dir.join("session.json"), "{broken").unwrap();
        assert_eq!(load(&dir), Session::default());
        assert!(record(
            &dir,
            Entry {
                result: "NaN".into(),
                ..entry(1)
            }
        )
        .is_err());
        assert!(record(
            &dir,
            Entry {
                expression: "x".repeat(MAX_EXPRESSION + 1),
                ..entry(1)
            }
        )
        .is_err());
    }

    #[test]
    fn concurrent_windows_keep_each_others_history() {
        let dir = Scratch::new("calculator-windows");
        std::thread::scope(|scope| {
            for n in 0..8 {
                let path = &dir;
                scope.spawn(move || {
                    record(path, entry(n)).unwrap();
                });
            }
        });
        assert_eq!(load(&dir).history.len(), 8);
    }
}
