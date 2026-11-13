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

use serde::Serialize;

use crate::places::{percent_decode, percent_encode};

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

/// Moves `p` (absolute) to the trash; returns where it went.
pub fn put(p: &Path) -> Result<PathBuf, String> {
    put_with(p, &home_trash())
}

pub fn put_with(p: &Path, home_trash: &Path) -> Result<PathBuf, String> {
    if !p.is_absolute() {
        return Err("not an absolute path".into());
    }
    let (trash, relative_to) = trash_for(p, home_trash)?;
    for d in [trash.join("files"), trash.join("info")] {
        fs::create_dir_all(&d).map_err(|e| format!("can't use the trash here: {e}"))?;
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
    Err("the trash is full of things with this name".into())
}

/// Something in a Trash.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Trashed {
    /// Where it is now (in the trash's `files`).
    pub path: PathBuf,
    /// Where it came from.
    pub origin: PathBuf,
    /// When it was trashed, unix ms (local time, as the spec has it).
    pub deleted: i64,
}

/// Every Trash there is: the home one, and those on mounted drives.
pub fn all_trashes() -> Vec<PathBuf> {
    let mut v = vec![home_trash()];
    for d in crate::places::drives() {
        let top = Path::new(&d.path);
        for t in [
            top.join(".Trash").join(uid().to_string()),
            top.join(format!(".Trash-{}", uid())),
        ] {
            if t.join("files").is_dir() && !v.contains(&t) {
                v.push(t);
            }
        }
    }
    v
}

/// The drive top a Trash's `Path=` lines are relative to (None: absolute).
fn top_of(trash: &Path) -> Option<PathBuf> {
    let name = trash.file_name()?.to_string_lossy();
    if name.starts_with(".Trash-") {
        return trash.parent().map(Path::to_path_buf);
    }
    let parent = trash.parent()?;
    if parent.file_name()? == ".Trash" {
        return parent.parent().map(Path::to_path_buf);
    }
    None
}

/// "YYYY-MM-DDThh:mm:ss" (local) as unix ms.
fn parse_date(s: &str) -> Option<i64> {
    let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i32>().ok();
    // SAFETY: mktime reads a filled-in tm.
    unsafe {
        let mut tm: libc::tm = std::mem::zeroed();
        tm.tm_year = n(0..4)? - 1900;
        tm.tm_mon = n(5..7)? - 1;
        tm.tm_mday = n(8..10)?;
        tm.tm_hour = n(11..13)?;
        tm.tm_min = n(14..16)?;
        tm.tm_sec = n(17..19)?;
        tm.tm_isdst = -1;
        let t = libc::mktime(&mut tm);
        (t != -1).then_some(t as i64 * 1000)
    }
}

/// (original path, deleted ms) from a .trashinfo's text.
fn parse_info(text: &str, top: Option<&Path>) -> Option<(PathBuf, i64)> {
    if text.lines().next()?.trim() != "[Trash Info]" {
        return None;
    }
    let mut path = None;
    let mut date = 0;
    for l in text.lines() {
        if let Some(p) = l.strip_prefix("Path=") {
            let p = PathBuf::from(percent_decode(p.trim()));
            path = Some(match top {
                Some(t) if p.is_relative() => t.join(p),
                _ => p,
            });
        } else if let Some(d) = l.strip_prefix("DeletionDate=") {
            date = parse_date(d.trim()).unwrap_or(0);
        }
    }
    path.filter(|p| p.is_absolute()).map(|p| (p, date))
}

/// What's in one Trash (items whose .trashinfo is missing or broken are left out).
pub fn list_in(trash: &Path) -> Vec<Trashed> {
    let top = top_of(trash);
    let Ok(rd) = fs::read_dir(trash.join("info")) else {
        return vec![];
    };
    let mut out = vec![];
    for e in rd.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(stem) = name.strip_suffix(".trashinfo") else {
            continue;
        };
        let file = trash.join("files").join(stem);
        if fs::symlink_metadata(&file).is_err() {
            continue;
        }
        let Ok(text) = fs::read_to_string(e.path()) else {
            continue;
        };
        if let Some((origin, deleted)) = parse_info(&text, top.as_deref()) {
            out.push(Trashed {
                path: file,
                origin,
                deleted,
            });
        }
    }
    out
}

pub fn list() -> Vec<Trashed> {
    all_trashes().iter().flat_map(|t| list_in(t)).collect()
}

/// The Trash `p` is in the `files` of, if it's a trashed item (not
/// something inside a trashed folder).
fn trash_of(p: &Path) -> Option<PathBuf> {
    let files = p.parent()?;
    (files.file_name()? == "files").then(|| files.parent().map(Path::to_path_buf))?
}

/// A trashed item's .trashinfo.
pub fn info_of(p: &Path) -> Option<PathBuf> {
    let t = trash_of(p)?;
    let info = t
        .join("info")
        .join(format!("{}.trashinfo", p.file_name()?.to_string_lossy()));
    info.is_file().then_some(info)
}

/// Whether `p` is in a Trash (an item, or inside a trashed folder), and
/// the item's path from its Trash: ["photos", "2024"] for .../files/photos/2024.
pub fn inside(p: &Path) -> Option<(PathBuf, Vec<String>)> {
    for t in all_trashes() {
        if let Ok(rest) = p.strip_prefix(t.join("files")) {
            let parts: Vec<String> = rest
                .iter()
                .map(|c| c.to_string_lossy().into_owned())
                .collect();
            if !parts.is_empty() {
                return Some((t, parts));
            }
        }
    }
    None
}

/// Puts a trashed item back where it came from (renamed if something's
/// there now; the folder made again if it's gone). Returns where it went.
pub fn restore(p: &Path) -> Result<PathBuf, String> {
    let info = info_of(p).ok_or("this isn't something in the trash")?;
    let text = fs::read_to_string(&info).map_err(|e| e.to_string())?;
    let trash = trash_of(p).ok_or("not in a Trash")?;
    let (origin, _) =
        parse_info(&text, top_of(&trash).as_deref()).ok_or("its Trash record is broken")?;
    let dir = origin.parent().ok_or("it came from nowhere")?;
    fs::create_dir_all(dir).map_err(|e| format!("can't make {}: {e}", dir.display()))?;
    let name = origin
        .file_name()
        .ok_or("no name")?
        .to_string_lossy()
        .into_owned();
    let target = if fs::symlink_metadata(&origin).is_ok() {
        crate::ops::free_name(dir, &name)
    } else {
        origin.clone()
    };
    fs::rename(p, &target).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(info);
    Ok(target)
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

    #[test]
    fn listed_and_restored() {
        let d = scratch("restore");
        let trash = d.join("Trash");
        fs::create_dir_all(d.join("work")).unwrap();
        fs::write(d.join("work/a.txt"), "a").unwrap();
        let went = put_with(&d.join("work/a.txt"), &trash).unwrap();
        let items = list_in(&trash);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].path, went);
        assert_eq!(items[0].origin, d.join("work/a.txt"));
        let age = (now_ms() - items[0].deleted).abs();
        assert!(age < 5_000, "deleted just now ({age} ms off)");

        // Its folder's gone, and something's taken its name: both handled.
        fs::remove_dir(d.join("work")).unwrap();
        let back = restore(&went).unwrap();
        assert_eq!(back, d.join("work/a.txt"));
        assert_eq!(fs::read_to_string(&back).unwrap(), "a");
        assert!(list_in(&trash).is_empty(), "its .trashinfo went too");

        let went = put_with(&back, &trash).unwrap();
        fs::write(&back, "new").unwrap();
        assert_eq!(restore(&went).unwrap(), d.join("work/a (2).txt"));
    }

    #[test]
    fn relative_paths_and_broken_records() {
        let top = Path::new("/media/usb");
        assert_eq!(
            parse_info(
                "[Trash Info]\nPath=photos/a%20b.jpg\nDeletionDate=2026-11-09T10:00:00\n",
                Some(top)
            )
            .map(|x| x.0),
            Some(PathBuf::from("/media/usb/photos/a b.jpg"))
        );
        assert_eq!(parse_info("nonsense", None), None);
        assert_eq!(
            parse_info("[Trash Info]\nPath=relative\n", None),
            None,
            "relative without a drive top"
        );
        assert_eq!(
            top_of(Path::new("/media/usb/.Trash-1000")),
            Some(PathBuf::from("/media/usb"))
        );
        assert_eq!(
            top_of(Path::new("/media/usb/.Trash/1000")),
            Some(PathBuf::from("/media/usb"))
        );
        assert_eq!(top_of(Path::new("/home/me/.local/share/Trash")), None);
        assert!(
            restore(Path::new("/etc/hostname")).is_err(),
            "only things in a Trash"
        );
    }

    fn now_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64
    }
}
