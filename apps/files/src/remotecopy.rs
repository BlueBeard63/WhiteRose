//! Local copies of files on network drives, for the preview. Reading a
//! file through a GVfs mount's FUSE folder is slow (a request at a time,
//! and a PDF reader jumps around in it), and the preview's readers run on
//! the window's thread: a PDF on a Nextcloud froze Files. So the file is
//! copied once, in the background, through GVfs directly (`gio copy` on
//! its own address), and the preview reads the copy.
//!
//! Kept in ~/.cache/whiterose/files/remote, named by the file's path, size
//! and time (a changed file is copied again); 500 MB at most, oldest
//! dropped first. Files over 50 MB aren't copied.

use std::{
    fs,
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use sha1::{Digest, Sha1};

const BIGGEST: u64 = 50 << 20;
const KEEP: u64 = 500 << 20;

pub fn dir() -> PathBuf {
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| crate::places::home().join(".cache"));
    cache.join("whiterose/files/remote")
}

/// The copy's name: the path's hash, its size and time, and its extension
/// (readers go by it).
pub fn name_for(path: &str, size: u64, mtime: i64) -> String {
    let h = Sha1::digest(path.as_bytes());
    let hex: String = h.iter().take(10).map(|b| format!("{b:02x}")).collect();
    let ext = Path::new(path)
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    format!("{hex}-{size}-{mtime}{ext}")
}

/// A local copy of `path` (made if need be).
pub fn fetch(path: &str) -> Result<String, String> {
    fetch_into(path, &dir())
}

fn fetch_into(path: &str, d: &Path) -> Result<String, String> {
    let m = fs::metadata(path).map_err(|e| e.to_string())?;
    if !m.is_file() {
        return Err("not a file".into());
    }
    if m.size() > BIGGEST {
        return Err("big".into());
    }
    fs::create_dir_all(d).map_err(|e| e.to_string())?;
    let local = d.join(name_for(path, m.size(), m.mtime()));
    if local.exists() {
        // (Used again: newest, for the pruning.)
        let _ = fs::File::options()
            .write(true)
            .open(&local)
            .and_then(|f| f.set_modified(std::time::SystemTime::now()));
        return Ok(local.to_string_lossy().into_owned());
    }
    let tmp = d.join(format!(
        ".part-{}-{}",
        std::process::id(),
        name_for(path, 0, 0)
    ));
    let run = std::env::var_os("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_default();
    // Through GVfs directly when it's a GVfs mount; else a plain copy.
    let through_gio = crate::places::gvfs_uri(path, &run).is_some_and(|uri| {
        Command::new("gio")
            .args(["copy", &uri, &tmp.to_string_lossy()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|s| s.success())
    });
    if !through_gio {
        if let Err(e) = fs::copy(path, &tmp) {
            let _ = fs::remove_file(&tmp);
            return Err(e.to_string());
        }
    }
    fs::rename(&tmp, &local).map_err(|e| e.to_string())?;
    prune(d, KEEP);
    Ok(local.to_string_lossy().into_owned())
}

/// Drops the oldest copies until `d` holds no more than `keep` bytes.
pub fn prune(d: &Path, keep: u64) {
    let Ok(rd) = fs::read_dir(d) else { return };
    let mut files: Vec<(std::time::SystemTime, u64, PathBuf)> = rd
        .flatten()
        .filter(|e| !e.file_name().to_string_lossy().starts_with(".part-"))
        .filter_map(|e| {
            let m = e.metadata().ok()?;
            Some((m.modified().ok()?, m.len(), e.path()))
        })
        .collect();
    let mut total: u64 = files.iter().map(|f| f.1).sum();
    files.sort_by_key(|f| f.0);
    for (_, len, p) in files {
        if total <= keep {
            break;
        }
        if fs::remove_file(&p).is_ok() {
            total -= len;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scratch::Scratch;

    #[test]
    fn names_change_with_the_file() {
        let a = name_for("/run/user/1000/gvfs/dav:host=x/Uni/a.pdf", 100, 5);
        assert!(a.ends_with("-100-5.pdf"), "{a}");
        assert_ne!(
            a,
            name_for("/run/user/1000/gvfs/dav:host=x/Uni/a.pdf", 101, 5)
        );
        assert_ne!(
            a,
            name_for("/run/user/1000/gvfs/dav:host=x/Uni/b.pdf", 100, 5)
        );
        assert!(!name_for("/x/README", 1, 1).contains('.'));
    }

    #[test]
    fn copies_once_and_prunes() {
        let d = Scratch::new("remotecopy");
        let src = d.join("src.txt");
        fs::write(&src, "hello").unwrap();
        let cache = d.join("cache");
        let a = fetch_into(&src.to_string_lossy(), &cache).unwrap();
        assert_eq!(fs::read_to_string(&a).unwrap(), "hello");
        assert_eq!(
            fetch_into(&src.to_string_lossy(), &cache).unwrap(),
            a,
            "copied once"
        );
        // Pruning to 0 bytes leaves nothing but a copy still being written.
        fs::write(cache.join(".part-1"), "x").unwrap();
        prune(&cache, 0);
        let left: Vec<_> = fs::read_dir(&cache)
            .unwrap()
            .flatten()
            .map(|e| e.file_name())
            .collect();
        assert_eq!(left, vec![std::ffi::OsString::from(".part-1")]);
        assert!(
            fetch_into(&d.to_string_lossy(), &cache).is_err(),
            "a folder isn't copied"
        );
    }
}
