//! Extracting archives safely: zip, and tar on its own or compressed (gz,
//! xz, bz2, zst). Everything goes into a new folder beside the archive
//! (named after it; an archive holding a single folder becomes that folder),
//! and nothing in the archive can reach outside it:
//!
//! - names that are absolute or climb out (`..`) are refused, and so is the
//!   whole archive if any are there (it was made to do harm);
//! - links may only point inside the new folder; nothing is ever written
//!   through a link;
//! - devices, pipes and hard links aren't made; setuid / setgid bits go;
//! - it stops before the drive is nearly full (archives that blow up), and
//!   when cancelled, leaving nothing half-made behind.

use std::{
    fs,
    io::{self, Read, Write},
    os::unix::fs::{symlink, PermissionsExt},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Format {
    Zip,
    Tar,
    TarGz,
    TarXz,
    TarBz2,
    TarZst,
}

/// The format from the name (None: not one Files extracts).
pub fn format_of(name: &str) -> Option<Format> {
    let n = name.to_lowercase();
    let ends = |e: &[&str]| e.iter().any(|x| n.ends_with(x));
    Some(if ends(&[".zip", ".jar"]) {
        Format::Zip
    } else if ends(&[".tar.gz", ".tgz"]) {
        Format::TarGz
    } else if ends(&[".tar.xz", ".txz"]) {
        Format::TarXz
    } else if ends(&[".tar.bz2", ".tbz2", ".tbz"]) {
        Format::TarBz2
    } else if ends(&[".tar.zst", ".tzst"]) {
        Format::TarZst
    } else if ends(&[".tar"]) {
        Format::Tar
    } else {
        return None;
    })
}

/// The archive's name without its extension: "photos" for photos.tar.gz.
pub fn stem(name: &str) -> String {
    let n = name.to_lowercase();
    for e in [
        ".tar.gz", ".tar.xz", ".tar.bz2", ".tar.zst", ".tgz", ".txz", ".tbz2", ".tbz", ".tzst",
        ".tar", ".zip", ".jar",
    ] {
        if n.ends_with(e) && name.len() > e.len() {
            return name[..name.len() - e.len()].to_string();
        }
    }
    name.to_string()
}

/// An entry's name as a path inside the new folder, or why it's refused.
pub fn safe_path(name: &str) -> Result<Option<PathBuf>, String> {
    // Zip names written on Windows use backslashes.
    let name = name.replace('\\', "/");
    let p = Path::new(&name);
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::Normal(x) => out.push(x),
            Component::CurDir => {}
            Component::ParentDir => return Err(format!("“{name}” climbs out of the folder")),
            Component::RootDir | Component::Prefix(_) => {
                return Err(format!("“{name}” is an absolute path"))
            }
        }
    }
    // "./" on its own: nothing to make.
    Ok((!out.as_os_str().is_empty()).then_some(out))
}

/// A link at `at` (inside the new folder) pointing to `target` stays inside it.
pub fn link_stays_inside(at: &Path, target: &Path) -> bool {
    if target.is_absolute() {
        return false;
    }
    let mut depth: i64 = at
        .parent()
        .map(|p| p.components().count() as i64)
        .unwrap_or(0);
    for c in target.components() {
        match c {
            Component::ParentDir => {
                depth -= 1;
                if depth < 0 {
                    return false;
                }
            }
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            _ => return false,
        }
    }
    true
}

/// What's happening, for the progress card.
pub trait Progress {
    /// Archive bytes read of the whole, and the entry being made.
    fn at(&mut self, done: u64, total: u64, entry: &str);
    /// False: stop (cancelled).
    fn go_on(&mut self) -> bool;
}

/// The archive read through this counts how far it's got.
struct Counting<R> {
    inner: R,
    read: Arc<AtomicU64>,
}

impl<R: io::Seek> io::Seek for Counting<R> {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        // (Zip reads from all over: where it is, rather than how much read.)
        let at = self.inner.seek(pos)?;
        self.read.store(at, Ordering::Relaxed);
        Ok(at)
    }
}

impl<R: Read> Read for Counting<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.read.fetch_add(n as u64, Ordering::Relaxed);
        Ok(n)
    }
}

fn free_space(p: &Path) -> u64 {
    use std::os::unix::ffi::OsStrExt;
    let Ok(c) = std::ffi::CString::new(p.as_os_str().as_bytes()) else {
        return u64::MAX;
    };
    // SAFETY: statvfs fills `s` for a valid path.
    unsafe {
        let mut s: libc::statvfs = std::mem::zeroed();
        if libc::statvfs(c.as_ptr(), &mut s) != 0 {
            return u64::MAX;
        }
        s.f_bavail as u64 * s.f_frsize as u64
    }
}

/// Writing into the new folder: refuses anything that would go through a
/// link, and stops short of filling the drive.
struct Out<'a> {
    root: &'a Path,
    room: u64,
    written: u64,
    skipped: Vec<(String, String)>,
}

impl Out<'_> {
    /// Makes the folders down to `rel`'s parent, each a real folder (not a link).
    fn dirs_for(&self, rel: &Path) -> Result<PathBuf, String> {
        let mut at = self.root.to_path_buf();
        if let Some(parent) = rel.parent() {
            for c in parent.components() {
                at.push(c);
                match fs::symlink_metadata(&at) {
                    Ok(m) if m.is_dir() => {}
                    Ok(_) => {
                        return Err(format!(
                            "“{}” would be written through a link",
                            rel.display()
                        ))
                    }
                    Err(_) => fs::create_dir(&at).map_err(|e| e.to_string())?,
                }
            }
        }
        Ok(self.root.join(rel))
    }

    fn file(
        &mut self,
        rel: &Path,
        mode: u32,
        data: &mut dyn Read,
        prog: &mut dyn Progress,
    ) -> Result<bool, String> {
        let path = self.dirs_for(rel)?;
        let mut f = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .map_err(|e| match e.kind() {
                io::ErrorKind::AlreadyExists => {
                    format!("“{}” is in the archive twice", rel.display())
                }
                _ => e.to_string(),
            })?;
        let mut buf = vec![0u8; 1 << 16];
        loop {
            if !prog.go_on() {
                return Ok(false);
            }
            let n = data
                .read(&mut buf)
                .map_err(|e| format!("the archive is damaged: {e}"))?;
            if n == 0 {
                break;
            }
            self.written += n as u64;
            if self.written > self.room {
                return Err(
                    "the drive is nearly full (the archive is bigger unpacked than the space left)"
                        .into(),
                );
            }
            f.write_all(&buf[..n]).map_err(|e| e.to_string())?;
        }
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode(mode & 0o777));
        Ok(true)
    }

    fn dir(&mut self, rel: &Path, mode: u32) -> Result<(), String> {
        let path = self.dirs_for(rel)?;
        match fs::symlink_metadata(&path) {
            Ok(m) if m.is_dir() => {}
            Ok(_) => return Err(format!("“{}” is in the archive twice", rel.display())),
            Err(_) => fs::create_dir(&path).map_err(|e| e.to_string())?,
        }
        // (Owner can always get in, whatever the archive says.)
        let _ = fs::set_permissions(&path, fs::Permissions::from_mode((mode & 0o777) | 0o700));
        Ok(())
    }

    fn link(&mut self, rel: &Path, target: &Path) -> Result<(), String> {
        if !link_stays_inside(rel, target) {
            self.skipped.push((
                rel.display().to_string(),
                format!("a link to {} (outside the folder)", target.display()),
            ));
            return Ok(());
        }
        let path = self.dirs_for(rel)?;
        symlink(target, &path).map_err(|e| e.to_string())
    }
}

/// Every name in the archive checked first: any refused, nothing's made.
fn check_names(names: &[String]) -> Result<(), String> {
    for n in names {
        safe_path(n)?;
    }
    Ok(())
}

fn tar_reader(fmt: Format, f: Counting<fs::File>) -> Result<Box<dyn Read>, String> {
    let r: Box<dyn Read> = match fmt {
        Format::Tar => Box::new(f),
        Format::TarGz => Box::new(flate2::read::MultiGzDecoder::new(f)),
        Format::TarXz => Box::new(xz2::read::XzDecoder::new_multi_decoder(f)),
        Format::TarBz2 => Box::new(bzip2::read::MultiBzDecoder::new(f)),
        Format::TarZst => Box::new(zstd::stream::read::Decoder::new(f).map_err(|e| e.to_string())?),
        Format::Zip => unreachable!(),
    };
    Ok(r)
}

fn tar_names(fmt: Format, archive: &Path) -> Result<Vec<String>, String> {
    let f = Counting {
        inner: fs::File::open(archive).map_err(|e| e.to_string())?,
        read: Arc::default(),
    };
    let mut t = tar::Archive::new(tar_reader(fmt, f)?);
    let mut v = vec![];
    for e in t
        .entries()
        .map_err(|e| format!("not a readable archive: {e}"))?
    {
        let e = e.map_err(|e| format!("the archive is damaged: {e}"))?;
        v.push(String::from_utf8_lossy(&e.path_bytes()).into_owned());
    }
    Ok(v)
}

fn unpack(
    fmt: Format,
    archive: &Path,
    into: &Path,
    prog: &mut dyn Progress,
    out: &mut Out,
) -> Result<bool, String> {
    let total = fs::metadata(archive).map(|m| m.len()).unwrap_or(0);
    let read = Arc::new(AtomicU64::new(0));
    let file = fs::File::open(archive).map_err(|e| e.to_string())?;
    let _ = into;
    if fmt == Format::Zip {
        let mut z = zip::ZipArchive::new(Counting {
            inner: file,
            read: read.clone(),
        })
        .map_err(|e| format!("not a readable zip: {e}"))?;
        let names: Vec<String> = (0..z.len())
            .filter_map(|i| z.by_index_raw(i).ok().map(|e| e.name().to_string()))
            .collect();
        check_names(&names)?;
        for i in 0..z.len() {
            if !prog.go_on() {
                return Ok(false);
            }
            let mut e = z
                .by_index(i)
                .map_err(|e| format!("the archive is damaged: {e}"))?;
            if e.encrypted() {
                return Err("it's protected with a password (not supported yet)".into());
            }
            let name = e.name().to_string();
            prog.at(read.load(Ordering::Relaxed), total, &name);
            let Some(rel) = safe_path(&name)? else {
                continue;
            };
            let mode = e
                .unix_mode()
                .unwrap_or(if e.is_dir() { 0o755 } else { 0o644 });
            if e.is_dir() {
                out.dir(&rel, mode)?;
            } else if mode & 0o170000 == 0o120000 {
                let mut t = String::new();
                e.read_to_string(&mut t).map_err(|e| e.to_string())?;
                out.link(&rel, Path::new(&t))?;
            } else if !out.file(&rel, mode, &mut e, prog)? {
                return Ok(false);
            }
        }
        return Ok(true);
    }
    // Tar: names checked in a first pass, then made in a second.
    check_names(&tar_names(fmt, archive)?)?;
    let mut t = tar::Archive::new(tar_reader(
        fmt,
        Counting {
            inner: file,
            read: read.clone(),
        },
    )?);
    for e in t
        .entries()
        .map_err(|e| format!("not a readable archive: {e}"))?
    {
        if !prog.go_on() {
            return Ok(false);
        }
        let mut e = e.map_err(|e| format!("the archive is damaged: {e}"))?;
        let name = String::from_utf8_lossy(&e.path_bytes()).into_owned();
        prog.at(read.load(Ordering::Relaxed), total, &name);
        let Some(rel) = safe_path(&name)? else {
            continue;
        };
        let mode = e.header().mode().unwrap_or(0o644);
        match e.header().entry_type() {
            tar::EntryType::Directory => out.dir(&rel, mode)?,
            tar::EntryType::Regular | tar::EntryType::Continuous | tar::EntryType::GNUSparse => {
                if !out.file(&rel, mode, &mut e, prog)? {
                    return Ok(false);
                }
            }
            tar::EntryType::Symlink => {
                let target = e
                    .link_name()
                    .map_err(|e| e.to_string())?
                    .unwrap_or_default()
                    .into_owned();
                out.link(&rel, &target)?;
            }
            // pax / GNU long-name headers are handled by the reader itself.
            tar::EntryType::XGlobalHeader
            | tar::EntryType::XHeader
            | tar::EntryType::GNULongName
            | tar::EntryType::GNULongLink => {}
            other => out.skipped.push((name, format!("not made ({other:?})"))),
        }
    }
    Ok(true)
}

/// The folder made, and what was skipped ([entry, why]).
pub type Extracted = (PathBuf, Vec<(String, String)>);

/// Extracts `archive` beside itself (or into `dest`): returns the folder
/// made and what was skipped ([entry, why]); Ok(None) when cancelled.
pub fn extract(
    archive: &Path,
    dest: Option<&Path>,
    prog: &mut dyn Progress,
) -> Result<Option<Extracted>, String> {
    let name = archive
        .file_name()
        .ok_or("no name")?
        .to_string_lossy()
        .into_owned();
    let fmt = format_of(&name).ok_or("not an archive Files can open")?;
    let dir = dest.or(archive.parent()).ok_or("no folder")?.to_path_buf();
    // Made under a hidden name, renamed into place when complete.
    let work = crate::ops::free_name(&dir, &format!(".{}.extracting", stem(&name)));
    fs::create_dir(&work).map_err(|e| format!("can't make a folder here: {e}"))?;
    let room = (free_space(&dir) as f64 * 0.95) as u64;
    let mut out = Out {
        root: &work,
        room,
        written: 0,
        skipped: vec![],
    };
    let done = unpack(fmt, archive, &dir, prog, &mut out);
    let skipped = std::mem::take(&mut out.skipped);
    match done {
        Ok(true) => {}
        Ok(false) => {
            let _ = fs::remove_dir_all(&work);
            return Ok(None);
        }
        Err(e) => {
            let _ = fs::remove_dir_all(&work);
            return Err(e);
        }
    }
    // One folder at the top: it becomes the result (not a folder in a folder).
    let top: Vec<PathBuf> = fs::read_dir(&work)
        .map_err(|e| e.to_string())?
        .flatten()
        .map(|e| e.path())
        .collect();
    let single = (top.len() == 1
        && fs::symlink_metadata(&top[0])
            .map(|m| m.is_dir())
            .unwrap_or(false))
    .then(|| top[0].clone());
    let (from, want) = match &single {
        Some(d) => (
            d.clone(),
            d.file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .into_owned(),
        ),
        None => (work.clone(), stem(&name)),
    };
    let target = if fs::symlink_metadata(dir.join(&want)).is_ok() {
        crate::ops::free_name(&dir, &want)
    } else {
        dir.join(&want)
    };
    fs::rename(&from, &target).map_err(|e| e.to_string())?;
    if single.is_some() {
        let _ = fs::remove_dir(&work);
    }
    Ok(Some((target, skipped)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;

    struct Quiet(u32, Option<u32>);
    impl Progress for Quiet {
        fn at(&mut self, _: u64, _: u64, _: &str) {}
        fn go_on(&mut self) -> bool {
            self.0 += 1;
            self.1.is_none_or(|stop| self.0 < stop)
        }
    }

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("wr-archive-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// A tar.gz with raw entries ([name, kind, data or link target, mode]).
    fn targz(path: &Path, entries: &[(&str, char, &str, u32)]) {
        let f = fs::File::create(path).unwrap();
        let gz = flate2::write::GzEncoder::new(f, flate2::Compression::fast());
        let mut b = tar::Builder::new(gz);
        for (name, kind, data, mode) in entries {
            let mut h = tar::Header::new_gnu();
            // Written raw so the harmful names go in as they are.
            let bytes = name.as_bytes();
            h.as_old_mut().name[..bytes.len()].copy_from_slice(bytes);
            h.set_mode(*mode);
            match kind {
                'd' => {
                    h.set_entry_type(tar::EntryType::Directory);
                    h.set_size(0);
                }
                'l' => {
                    h.set_entry_type(tar::EntryType::Symlink);
                    h.set_link_name(data).unwrap();
                    h.set_size(0);
                }
                _ => {
                    h.set_entry_type(tar::EntryType::Regular);
                    h.set_size(data.len() as u64);
                }
            }
            h.set_cksum();
            let body: &[u8] = if *kind == 'f' { data.as_bytes() } else { &[] };
            b.append(&h, body).unwrap();
        }
        b.into_inner().unwrap().finish().unwrap();
    }

    #[test]
    fn names_and_paths() {
        assert_eq!(format_of("a.tar.gz"), Some(Format::TarGz));
        assert_eq!(format_of("A.ZIP"), Some(Format::Zip));
        assert_eq!(format_of("notes.txt"), None);
        assert_eq!(stem("photos.tar.gz"), "photos");
        assert_eq!(stem("x.zip"), "x");
        assert_eq!(safe_path("a/./b").unwrap(), Some(PathBuf::from("a/b")));
        assert!(safe_path("../etc/passwd").is_err());
        assert!(safe_path("a/../../b").is_err());
        assert!(safe_path("/etc/passwd").is_err());
        assert!(safe_path("..\\windows\\x").is_err(), "backslashes too");
        assert_eq!(safe_path("./").unwrap(), None);
        assert!(link_stays_inside(Path::new("a/b/link"), Path::new("../c")));
        assert!(!link_stays_inside(
            Path::new("a/link"),
            Path::new("../../etc")
        ));
        assert!(!link_stays_inside(
            Path::new("link"),
            Path::new("/etc/passwd")
        ));
    }

    #[test]
    fn a_good_archive() {
        let d = scratch("good");
        let a = d.join("photos.tar.gz");
        targz(
            &a,
            &[
                ("photos/", 'd', "", 0o755),
                ("photos/a.jpg", 'f', "AAA", 0o4755),
                ("photos/sub/b.jpg", 'f', "B", 0o644),
                ("photos/latest", 'l', "a.jpg", 0o777),
                ("photos/escape", 'l', "../../outside", 0o777),
            ],
        );
        let (made, skipped) = extract(&a, None, &mut Quiet(0, None)).unwrap().unwrap();
        assert_eq!(made, d.join("photos"), "one folder at the top: that folder");
        assert_eq!(fs::read_to_string(made.join("sub/b.jpg")).unwrap(), "B");
        assert_eq!(
            fs::metadata(made.join("a.jpg")).unwrap().mode() & 0o7777,
            0o755,
            "setuid gone"
        );
        assert_eq!(
            fs::read_link(made.join("latest")).unwrap(),
            PathBuf::from("a.jpg")
        );
        assert!(
            fs::symlink_metadata(made.join("escape")).is_err(),
            "a link outside isn't made"
        );
        assert_eq!(skipped.len(), 1);
        // Again: beside it, numbered.
        let (again, _) = extract(&a, None, &mut Quiet(0, None)).unwrap().unwrap();
        assert_eq!(again, d.join("photos (2)"));
        assert!(
            fs::read_dir(&d)
                .unwrap()
                .flatten()
                .all(|e| !e.file_name().to_string_lossy().contains("extracting")),
            "no leftovers"
        );
    }

    #[test]
    fn harmful_archives_are_refused_whole() {
        let d = scratch("evil");
        let a = d.join("evil.tar.gz");
        targz(
            &a,
            &[
                ("ok.txt", 'f', "fine", 0o644),
                ("../../evil.txt", 'f', "pwned", 0o644),
            ],
        );
        let e = extract(&a, None, &mut Quiet(0, None)).unwrap_err();
        assert!(e.contains("climbs out"), "{e}");
        assert!(
            !d.join("ok.txt").exists() && !d.join("evil").exists(),
            "nothing made"
        );
        assert!(!d.parent().unwrap().join("evil.txt").exists());

        // Writing through a link made earlier in the same archive.
        let b = d.join("sneaky.tar.gz");
        targz(
            &b,
            &[("dir", 'l', ".", 0o777), ("dir/../x", 'f', "x", 0o644)],
        );
        assert!(extract(&b, None, &mut Quiet(0, None)).is_err());
        let c = d.join("through.tar.gz");
        targz(
            &c,
            &[
                ("out", 'l', "sub", 0o777),
                ("sub/", 'd', "", 0o755),
                ("out/f.txt", 'f', "x", 0o644),
            ],
        );
        let e = extract(&c, None, &mut Quiet(0, None)).unwrap_err();
        assert!(e.contains("through a link"), "{e}");
    }

    #[test]
    fn zips_and_cancelling() {
        let d = scratch("zip");
        let a = d.join("docs.zip");
        {
            let mut z = zip::ZipWriter::new(fs::File::create(&a).unwrap());
            let o = zip::write::SimpleFileOptions::default();
            z.start_file("one.txt", o).unwrap();
            z.write_all(b"1").unwrap();
            z.start_file("two.txt", o).unwrap();
            z.write_all(b"2").unwrap();
            z.finish().unwrap();
        }
        let (made, _) = extract(&a, None, &mut Quiet(0, None)).unwrap().unwrap();
        assert_eq!(
            made,
            d.join("docs"),
            "several things at the top: a folder named after it"
        );
        assert_eq!(fs::read_to_string(made.join("two.txt")).unwrap(), "2");
        assert!(
            extract(&a, None, &mut Quiet(0, Some(2))).unwrap().is_none(),
            "cancelled"
        );
        assert!(!d.join("docs (2)").exists(), "nothing left when cancelled");
        let bad = d.join("bad.zip");
        {
            let mut z = zip::ZipWriter::new(fs::File::create(&bad).unwrap());
            z.start_file("../up.txt", zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(b"x").unwrap();
            z.finish().unwrap();
        }
        assert!(extract(&bad, None, &mut Quiet(0, None)).is_err());
    }
}
