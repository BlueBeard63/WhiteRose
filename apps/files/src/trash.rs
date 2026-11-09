//! The Trash, as the freedesktop.org spec has it (so other file managers
//! see the same Trash): files on the home drive go to
//! $XDG_DATA_HOME/Trash, files on another drive to that drive's
//! `.Trash/$uid` (if the admin made one) or `.Trash-$uid`. Each item gets a
//! `.trashinfo` with where it came from and when, so it can go back.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};

use crate::places::percent_encode;

/// The home Trash ($XDG_DATA_HOME/Trash).
pub fn home_trash() -> PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| crate::places::home().join(".local/share"))
        .join("Trash")
}

fn uid() -> u32 {
    // SAFETY: getuid can't fail.
    unsafe { libc::getuid() }
}

/// The top of the filesystem `p` is on: the highest folder on the same device.
fn mount_top(p: &Path) -> PathBuf {
    let Ok(dev) = fs::symlink_metadata(p).map(|m| m.dev()) else {
        return PathBuf::from("/");
    };
    let mut top = p.to_path_buf();
    while let Some(up) = top.parent() {
        match fs::metadata(up) {
            Ok(m) if m.dev() == dev => top = up.to_path_buf(),
            _ => break,
        }
    }
    top
}

/// Where `p` goes when trashed: (the trash folder, the folder its
/// `Path=` is relative to, or None for absolute).
fn trash_for(p: &Path, home: &Path) -> Result<(PathBuf, Option<PathBuf>), String> {
    let dev = fs::symlink_metadata(p).map_err(|e| e.to_string())?.dev();
    // The home Trash, when it's on the same drive (made if need be).
    let home_dev = fs::metadata(home)
        .or_else(|_| {
            home.parent()
                .map(fs::metadata)
                .unwrap_or_else(|| fs::metadata("/"))
        })
        .map(|m| m.dev());
    if home_dev.ok() == Some(dev) {
        return Ok((home.to_path_buf(), None));
    }
    let top = mount_top(p);
    // $top/.Trash/$uid: only if .Trash is a real folder with the sticky bit.
    let shared = top.join(".Trash");
    if let Ok(m) = fs::symlink_metadata(&shared) {
        if m.is_dir() && m.permissions().mode() & 0o1000 != 0 {
            return Ok((shared.join(uid().to_string()), Some(top)));
        }
    }
    Ok((top.join(format!(".Trash-{}", uid())), Some(top)))
}

/// Local time as the spec wants it (YYYY-MM-DDThh:mm:ss).
fn now_local() -> String {
    // SAFETY: localtime_r fills `tm` from a valid time_t.
    unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            tm.tm_year + 1900,
            tm.tm_mon + 1,
            tm.tm_mday,
            tm.tm_hour,
            tm.tm_min,
            tm.tm_sec
        )
    }
}

/// Moves `p` (absolute) to the Trash; returns where it went.
pub fn put(p: &Path) -> Result<PathBuf, String> {
    put_with(p, &home_trash())
}

pub fn put_with(p: &Path, home_trash: &Path) -> Result<PathBuf, String> {
    if !p.is_absolute() {
        return Err("not an absolute path".into());
    }
    let (trash, relative_to) = trash_for(p, home_trash)?;
    for d in [trash.join("files"), trash.join("info")] {
        fs::create_dir_all(&d).map_err(|e| format!("can't use the Trash here: {e}"))?;
    }
    if relative_to.is_some() {
        let _ = fs::set_permissions(&trash, fs::Permissions::from_mode(0o700));
    }
    let shown = match &relative_to {
        Some(top) => p.strip_prefix(top).unwrap_or(p).to_path_buf(),
        None => p.to_path_buf(),
    };
    let name = p
        .file_name()
        .ok_or("can't trash this")?
        .to_string_lossy()
        .into_owned();
    // Claim a name: creating the .trashinfo exclusively is the lock.
    for n in 1..10_000 {
        let candidate = if n == 1 {
            name.clone()
        } else {
            crate::ops::numbered(&name, n)
        };
        let info_path = trash.join("info").join(format!("{candidate}.trashinfo"));
        let target = trash.join("files").join(&candidate);
        if fs::symlink_metadata(&target).is_ok() {
            continue;
        }
        let mut info = match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&info_path)
        {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e.to_string()),
        };
        let text = format!(
            "[Trash Info]\nPath={}\nDeletionDate={}\n",
            percent_encode(&shown.to_string_lossy()),
            now_local()
        );
        if let Err(e) = info.write_all(text.as_bytes()) {
            let _ = fs::remove_file(&info_path);
            return Err(e.to_string());
        }
        return match fs::rename(p, &target) {
            Ok(()) => Ok(target),
            Err(e) => {
                let _ = fs::remove_file(&info_path);
                Err(e.to_string())
            }
        };
    }
    Err("the Trash is full of things with this name".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("wr-trash-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn trashing_records_where_it_came_from() {
        let d = scratch("put");
        let trash = d.join("Trash");
        let f = d.join("my notes.txt");
        fs::write(&f, "hi").unwrap();
        let went = put_with(&f, &trash).unwrap();
        assert!(!f.exists());
        assert_eq!(went, trash.join("files/my notes.txt"));
        assert_eq!(fs::read_to_string(&went).unwrap(), "hi");
        let info = fs::read_to_string(trash.join("info/my notes.txt.trashinfo")).unwrap();
        let mut lines = info.lines();
        assert_eq!(lines.next(), Some("[Trash Info]"));
        let path_line = format!("Path={}", percent_encode(&f.to_string_lossy()));
        assert_eq!(lines.next(), Some(path_line.as_str()));
        let date = lines.next().unwrap().strip_prefix("DeletionDate=").unwrap();
        assert_eq!(date.len(), 19);
        assert_eq!(&date[10..11], "T");
    }

    #[test]
    fn same_name_twice_gets_numbered() {
        let d = scratch("twice");
        let trash = d.join("Trash");
        for _ in 0..3 {
            fs::create_dir_all(d.join("dir/inner")).unwrap();
            put_with(&d.join("dir"), &trash).unwrap();
        }
        assert!(trash.join("files/dir/inner").is_dir());
        assert!(trash.join("files/dir (2)").is_dir());
        assert!(trash.join("info/dir (3).trashinfo").is_file());
    }

    #[test]
    fn a_missing_file_is_an_error() {
        let d = scratch("missing");
        assert!(put_with(&d.join("nope"), &d.join("Trash")).is_err());
        assert!(put_with(Path::new("relative"), &d.join("Trash")).is_err());
        assert!(
            !d.join("Trash/info").exists()
                || fs::read_dir(d.join("Trash/info")).unwrap().count() == 0
        );
    }
}
