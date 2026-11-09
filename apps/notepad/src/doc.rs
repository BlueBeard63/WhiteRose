//! Reading and saving text files, safely:
//! - only text: binary files (a NUL in the first 8 KiB) and anything over
//!   20 MiB are refused rather than mangled; non-UTF-8 is refused too;
//! - line endings are kept (a CRLF file stays CRLF);
//! - saving writes a new file next to the old one and renames it over, so a
//!   crash or full disk never leaves half a file, and keeps its permissions;
//! - the modification time tells when a file changed on disk underneath you.

use std::{
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

pub const MAX_SIZE: u64 = 20 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Eol {
    Lf,
    Crlf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Doc {
    /// The text with "\n" line endings (as the editor works).
    pub text: String,
    pub eol: Eol,
    /// Modification time (ms) when read.
    pub mtime: i64,
}

pub fn mtime(path: &Path) -> i64 {
    fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

pub fn read(path: &Path) -> Result<Doc, String> {
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    if meta.is_dir() {
        return Err("that's a folder".into());
    }
    if meta.len() > MAX_SIZE {
        return Err(format!(
            "it's {} MiB; Notepad opens files up to 20 MiB",
            meta.len() / (1024 * 1024)
        ));
    }
    let bytes = fs::read(path).map_err(|e| e.to_string())?;
    if bytes[..bytes.len().min(8192)].contains(&0) {
        return Err("it isn't a text file".into());
    }
    let text = String::from_utf8(bytes).map_err(|_| "it isn't UTF-8 text".to_string())?;
    let eol = if text.contains("\r\n") {
        Eol::Crlf
    } else {
        Eol::Lf
    };
    Ok(Doc {
        text: text.replace("\r\n", "\n"),
        eol,
        mtime: mtime(path),
    })
}

/// Saves `text` ("\n" endings) with `eol`; returns the new modification time.
pub fn write(path: &Path, text: &str, eol: &Eol) -> Result<i64, String> {
    let dir = path
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let name = path
        .file_name()
        .ok_or("no file name")?
        .to_string_lossy()
        .to_string();
    let tmp: PathBuf = dir.join(format!(".{name}.whiterose-saving"));
    let data = match eol {
        Eol::Lf => text.to_string(),
        Eol::Crlf => text.replace('\n', "\r\n"),
    };
    let result = (|| -> std::io::Result<()> {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(data.as_bytes())?;
        f.sync_all()?;
        // Keep the old file's permissions (an executable script stays one).
        if let Ok(old) = fs::metadata(path) {
            fs::set_permissions(&tmp, fs::Permissions::from_mode(old.permissions().mode()))?;
        }
        fs::rename(&tmp, path)
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(e.to_string());
    }
    Ok(mtime(path))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn dir() -> PathBuf {
        static N: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let d = std::env::temp_dir().join(format!(
            "wr-notepad-{}-{}",
            std::process::id(),
            N.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn keeps_line_endings_and_permissions() {
        let d = dir();
        let p = d.join("run.sh");
        fs::write(&p, "#!/bin/sh\r\necho hi\r\n").unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        let doc = read(&p).unwrap();
        assert_eq!(
            (doc.text.as_str(), &doc.eol),
            ("#!/bin/sh\necho hi\n", &Eol::Crlf)
        );
        write(&p, "#!/bin/sh\necho bye\n", &doc.eol).unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "#!/bin/sh\r\necho bye\r\n");
        assert_eq!(
            fs::metadata(&p).unwrap().permissions().mode() & 0o777,
            0o755
        );
        assert!(
            !d.join(".run.sh.whiterose-saving").exists(),
            "no temporary file left behind"
        );
        fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn refuses_what_it_cant_edit() {
        let d = dir();
        fs::write(d.join("bin"), b"\x7fELF\x00\x01").unwrap();
        fs::write(d.join("latin1"), b"caf\xe9\n").unwrap();
        assert_eq!(read(&d.join("bin")).unwrap_err(), "it isn't a text file");
        assert_eq!(read(&d.join("latin1")).unwrap_err(), "it isn't UTF-8 text");
        assert_eq!(read(&d).unwrap_err(), "that's a folder");
        assert!(read(&d.join("missing")).is_err());
        fs::remove_dir_all(d).unwrap();
    }

    #[test]
    fn new_files_are_created_and_mtime_moves() {
        let d = dir();
        let p = d.join("notes.md");
        let t1 = write(&p, "# hello\n", &Eol::Lf).unwrap();
        assert_eq!(read(&p).unwrap().text, "# hello\n");
        assert_eq!(t1, mtime(&p));
        fs::remove_dir_all(d).unwrap();
    }
}
