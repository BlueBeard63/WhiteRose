//! Archives: extracting safely, and making them. Zip (with or without an
//! AES password), 7z (likewise), and tar on its own or compressed (gz, xz,
//! bz2, zst); any of them split into parts (name.zip.001, .002...).
//! Passwords are asked for when needed and kept in memory only.
//!
//! Extracting: Everything goes into a new folder beside the archive
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
    os::unix::fs::{symlink, MetadataExt, PermissionsExt},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Format {
    Zip,
    SevenZ,
    Tar,
    TarGz,
    TarXz,
    TarBz2,
    TarZst,
}

/// The format from the name (None: not one Files extracts). The first part
/// of a split archive counts as the archive ("x.zip.001").
pub fn format_of(name: &str) -> Option<Format> {
    let n = name.to_lowercase();
    let n = n.strip_suffix(".001").unwrap_or(&n);
    let ends = |e: &[&str]| e.iter().any(|x| n.ends_with(x));
    Some(if ends(&[".zip", ".jar"]) {
        Format::Zip
    } else if ends(&[".7z"]) {
        Format::SevenZ
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
    let name = name.strip_suffix(".001").unwrap_or(name);
    let n = name.to_lowercase();
    for e in [
        ".tar.gz", ".tar.xz", ".tar.bz2", ".tar.zst", ".tgz", ".txz", ".tbz2", ".tbz", ".tzst",
        ".tar", ".zip", ".jar", ".7z",
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

// ---- passwords ----------------------------------------------------------------------------

/// What an extraction says when it needs a password (QML asks, then tries again).
pub const NEEDS_PASSWORD: &str = "it's protected with a password";
pub const WRONG_PASSWORD: &str = "that password isn't right";

static PASSWORDS: std::sync::Mutex<Option<std::collections::HashMap<PathBuf, String>>> =
    std::sync::Mutex::new(None);

/// The password for `archive` from now on (in memory, this run only).
pub fn set_password(archive: &Path, password: &str) {
    PASSWORDS
        .lock()
        .unwrap()
        .get_or_insert_with(Default::default)
        .insert(archive.to_path_buf(), password.to_string());
}

fn password(archive: &Path) -> Option<String> {
    PASSWORDS.lock().unwrap().as_ref()?.get(archive).cloned()
}

// ---- split archives ---------------------------------------------------------------------------

/// A split archive's parts in order (x.001, x.002, ... as long as they go
/// on), or just the file.
pub fn parts(archive: &Path) -> Vec<PathBuf> {
    let name = archive.to_string_lossy();
    let Some(base) = name.strip_suffix(".001") else {
        return vec![archive.to_path_buf()];
    };
    let mut v = vec![];
    for i in 1..10_000 {
        let p = PathBuf::from(format!("{base}.{i:03}"));
        if !p.is_file() {
            break;
        }
        v.push(p);
    }
    v
}

/// The parts read as one file.
struct Joined {
    files: Vec<(fs::File, u64)>,
    at: u64,
    len: u64,
}

impl Joined {
    fn open(parts: &[PathBuf]) -> io::Result<Joined> {
        let mut files = vec![];
        let mut len = 0;
        for p in parts {
            let f = fs::File::open(p)?;
            let n = f.metadata()?.len();
            len += n;
            files.push((f, n));
        }
        Ok(Joined { files, at: 0, len })
    }
}

impl Read for Joined {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        use io::Seek;
        let mut start = 0;
        for (f, n) in &mut self.files {
            if self.at < start + *n {
                f.seek(io::SeekFrom::Start(self.at - start))?;
                let want = buf.len().min((start + *n - self.at) as usize);
                let got = f.read(&mut buf[..want])?;
                self.at += got as u64;
                return Ok(got);
            }
            start += *n;
        }
        Ok(0)
    }
}

impl io::Seek for Joined {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        let to = match pos {
            io::SeekFrom::Start(n) => n as i64,
            io::SeekFrom::End(d) => self.len as i64 + d,
            io::SeekFrom::Current(d) => self.at as i64 + d,
        };
        if to < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "before the start",
            ));
        }
        self.at = to as u64;
        Ok(self.at)
    }
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

fn tar_reader(fmt: Format, f: Counting<Joined>) -> Result<Box<dyn Read>, String> {
    let r: Box<dyn Read> = match fmt {
        Format::Tar => Box::new(f),
        Format::TarGz => Box::new(flate2::read::MultiGzDecoder::new(f)),
        Format::TarXz => Box::new(xz2::read::XzDecoder::new_multi_decoder(f)),
        Format::TarBz2 => Box::new(bzip2::read::MultiBzDecoder::new(f)),
        Format::TarZst => Box::new(zstd::stream::read::Decoder::new(f).map_err(|e| e.to_string())?),
        Format::Zip | Format::SevenZ => unreachable!(),
    };
    Ok(r)
}

fn tar_names(fmt: Format, archive: &Path) -> Result<Vec<String>, String> {
    let f = Counting {
        inner: Joined::open(&parts(archive)).map_err(|e| e.to_string())?,
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

/// Which entries to write, and as what (None: not this one).
type Pick<'a> = &'a dyn Fn(&str) -> Option<String>;

fn unpack(
    fmt: Format,
    archive: &Path,
    into: &Path,
    prog: &mut dyn Progress,
    out: &mut Out,
    pick: Pick,
) -> Result<bool, String> {
    let file = Joined::open(&parts(archive)).map_err(|e| e.to_string())?;
    let total = file.len;
    let read = Arc::new(AtomicU64::new(0));
    let _ = into;
    if fmt == Format::SevenZ {
        return unpack_7z(archive, file, read, total, prog, out, pick);
    }
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
            let raw = z
                .by_index_raw(i)
                .map(|e| e.name().to_string())
                .unwrap_or_default();
            let Some(picked) = pick(&raw) else {
                continue;
            };
            let encrypted = z.by_index_raw(i).map(|e| e.encrypted()).unwrap_or(false);
            let pw = password(archive);
            let r = match (encrypted, &pw) {
                (true, None) => return Err(NEEDS_PASSWORD.into()),
                (true, Some(pw)) => z.by_index_decrypt(i, pw.as_bytes()),
                _ => z.by_index(i),
            };
            let mut e = r.map_err(|e| match e {
                zip::result::ZipError::InvalidPassword => WRONG_PASSWORD.to_string(),
                e => format!("the archive is damaged: {e}"),
            })?;
            let name = e.name().to_string();
            prog.at(read.load(Ordering::Relaxed), total, &name);
            let Some(rel) = safe_path(&picked)? else {
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
        let Some(picked) = pick(&name) else {
            continue;
        };
        prog.at(read.load(Ordering::Relaxed), total, &name);
        let Some(rel) = safe_path(&picked)? else {
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

fn sevenz_error(e: sevenz_rust::Error) -> String {
    match e {
        sevenz_rust::Error::PasswordRequired => NEEDS_PASSWORD.into(),
        sevenz_rust::Error::MaybeBadPassword(_) => WRONG_PASSWORD.into(),
        e => format!("not a readable 7z: {e}"),
    }
}

fn unpack_7z(
    archive: &Path,
    file: Joined,
    read: Arc<AtomicU64>,
    total: u64,
    prog: &mut dyn Progress,
    out: &mut Out,
    pick: Pick,
) -> Result<bool, String> {
    let pw = sevenz_rust::Password::from(password(archive).unwrap_or_default().as_str());
    let len = file.len;
    let mut z = sevenz_rust::SevenZReader::new(
        Counting {
            inner: file,
            read: read.clone(),
        },
        len,
        pw,
    )
    .map_err(sevenz_error)?;
    let names: Vec<String> = z
        .archive()
        .files
        .iter()
        .map(|e| e.name().to_string())
        .collect();
    check_names(&names)?;
    let mut stopped = false;
    let mut failed: Option<String> = None;
    z.for_each_entries(|e, data| {
        if !prog.go_on() {
            stopped = true;
            return Ok(false);
        }
        let name = e.name().to_string();
        let Some(picked) = pick(&name) else {
            return Ok(true);
        };
        prog.at(read.load(Ordering::Relaxed), total, &name);
        let rel = match safe_path(&picked) {
            Ok(Some(r)) => r,
            Ok(None) => return Ok(true),
            Err(why) => {
                failed = Some(why);
                return Ok(false);
            }
        };
        let r = if e.is_directory() {
            out.dir(&rel, 0o755).map(|_| true)
        } else {
            out.file(&rel, 0o644, data, prog)
        };
        match r {
            Ok(true) => Ok(true),
            Ok(false) => {
                stopped = true;
                Ok(false)
            }
            Err(why) => {
                failed = Some(why);
                Ok(false)
            }
        }
    })
    .map_err(sevenz_error)?;
    if let Some(why) = failed {
        return Err(why);
    }
    Ok(!stopped)
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
    let done = unpack(fmt, archive, &dir, prog, &mut out, &|n| Some(n.to_string()));
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

// ---- looking inside ---------------------------------------------------------------------------

/// An archive's insides as a place: "archive:/home/me/photos.zip!/2024".
pub const PREFIX: &str = "archive:";

/// "archive:<file>!/<inside>" → (file, inside: "" at the top, no slashes at the ends).
pub fn split_place(p: &str) -> Option<(PathBuf, String)> {
    let rest = p.strip_prefix(PREFIX)?;
    let (file, inside) = rest
        .split_once("!/")
        .unwrap_or((rest.trim_end_matches('!'), ""));
    Some((PathBuf::from(file), inside.trim_matches('/').to_string()))
}

pub fn place(archive: &Path, inside: &str) -> String {
    format!(
        "{PREFIX}{}!/{}",
        archive.display(),
        inside.trim_matches('/')
    )
}

#[derive(serde::Serialize, Debug, Clone, PartialEq)]
pub struct Item {
    /// Its path inside ("photos/2024/a.jpg").
    pub path: String,
    pub dir: bool,
    pub size: u64,
    /// unix ms (0: unknown)
    pub modified: i64,
}

fn tidy(name: &str) -> String {
    name.trim_start_matches("./").trim_matches('/').to_string()
}

fn zip_time(t: Option<zip::DateTime>) -> i64 {
    let Some(t) = t else { return 0 };
    let (y, m, d) = (t.year() as i64, t.month() as i64, t.day() as i64);
    let y2 = if m <= 2 { y - 1 } else { y };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let days = era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468;
    (days * 86_400 + t.hour() as i64 * 3600 + t.minute() as i64 * 60 + t.second() as i64) * 1000
}

/// Everything in the archive (folders implied by paths included).
fn read_items(archive: &Path) -> Result<Vec<Item>, String> {
    let name = archive
        .file_name()
        .ok_or("no name")?
        .to_string_lossy()
        .into_owned();
    let fmt = format_of(&name).ok_or("not an archive Files can open")?;
    let mut v: Vec<Item> = vec![];
    match fmt {
        Format::Zip => {
            let file = Joined::open(&parts(archive)).map_err(|e| e.to_string())?;
            let mut z =
                zip::ZipArchive::new(file).map_err(|e| format!("not a readable zip: {e}"))?;
            for i in 0..z.len() {
                let e = z
                    .by_index_raw(i)
                    .map_err(|e| format!("the archive is damaged: {e}"))?;
                v.push(Item {
                    path: tidy(e.name()),
                    dir: e.is_dir(),
                    size: e.size(),
                    modified: zip_time(e.last_modified()),
                });
            }
        }
        Format::SevenZ => {
            let file = Joined::open(&parts(archive)).map_err(|e| e.to_string())?;
            let len = file.len;
            let pw = sevenz_rust::Password::from(password(archive).unwrap_or_default().as_str());
            let z = sevenz_rust::SevenZReader::new(file, len, pw).map_err(sevenz_error)?;
            for e in &z.archive().files {
                let raw = e.last_modified_date().to_raw() as i64;
                v.push(Item {
                    path: tidy(e.name()),
                    dir: e.is_directory(),
                    size: e.size(),
                    modified: if raw > 0 {
                        raw / 10_000 - 11_644_473_600_000
                    } else {
                        0
                    },
                });
            }
        }
        tar_fmt => {
            let f = Counting {
                inner: Joined::open(&parts(archive)).map_err(|e| e.to_string())?,
                read: Arc::default(),
            };
            let mut t = tar::Archive::new(tar_reader(tar_fmt, f)?);
            for e in t
                .entries()
                .map_err(|e| format!("not a readable archive: {e}"))?
            {
                let e = e.map_err(|e| format!("the archive is damaged: {e}"))?;
                let h = e.header();
                let kind = h.entry_type();
                if !(kind.is_file() || kind.is_dir() || kind.is_symlink()) {
                    continue;
                }
                v.push(Item {
                    path: tidy(&String::from_utf8_lossy(&e.path_bytes())),
                    dir: kind.is_dir(),
                    size: h.size().unwrap_or(0),
                    modified: h.mtime().unwrap_or(0) as i64 * 1000,
                });
            }
        }
    }
    // Folders only implied by what's in them.
    let mut known: std::collections::HashSet<String> = v.iter().map(|i| i.path.clone()).collect();
    let mut more = vec![];
    for i in &v {
        let mut p = i.path.as_str();
        while let Some((up, _)) = p.rsplit_once('/') {
            if known.insert(up.to_string()) {
                more.push(Item {
                    path: up.to_string(),
                    dir: true,
                    size: 0,
                    modified: i.modified,
                });
            }
            p = up;
        }
    }
    v.extend(more);
    v.retain(|i| !i.path.is_empty() && safe_path(&i.path).is_ok());
    Ok(v)
}

type Listing = (PathBuf, u64, i64, std::sync::Arc<Vec<Item>>);
static LISTED: std::sync::Mutex<Vec<Listing>> = std::sync::Mutex::new(Vec::new());

/// Everything in the archive, read once while it doesn't change (a big
/// compressed tar reads all the way through to list).
pub fn items(archive: &Path) -> Result<std::sync::Arc<Vec<Item>>, String> {
    let m = fs::metadata(archive).map_err(|e| e.to_string())?;
    let stamp = (
        m.len(),
        m.modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0),
    );
    {
        let l = LISTED.lock().unwrap();
        if let Some(hit) = l
            .iter()
            .find(|x| x.0 == archive && x.1 == stamp.0 && x.2 == stamp.1)
        {
            return Ok(hit.3.clone());
        }
    }
    let v = std::sync::Arc::new(read_items(archive)?);
    let mut l = LISTED.lock().unwrap();
    l.retain(|x| x.0 != archive);
    l.push((archive.to_path_buf(), stamp.0, stamp.1, v.clone()));
    if l.len() > 4 {
        l.remove(0);
    }
    Ok(v)
}

/// What's directly in `inside` ("" the top).
pub fn list(archive: &Path, inside: &str) -> Result<Vec<Item>, String> {
    let all = items(archive)?;
    let inside = inside.trim_matches('/');
    Ok(all
        .iter()
        .filter(|i| {
            let parent = i.path.rsplit_once('/').map(|(p, _)| p).unwrap_or("");
            parent == inside
        })
        .cloned()
        .collect())
}

/// Takes `chosen` (paths inside, all in one folder of it) out into `into`,
/// each beside the others as it was: a folder with everything in it.
/// Names already there get a number. The paths made; None if cancelled.
pub fn extract_some(
    archive: &Path,
    chosen: &[String],
    into: &Path,
    prog: &mut dyn Progress,
) -> Result<Option<Vec<PathBuf>>, String> {
    let name = archive
        .file_name()
        .ok_or("no name")?
        .to_string_lossy()
        .into_owned();
    let fmt = format_of(&name).ok_or("not an archive Files can open")?;
    let chosen: Vec<String> = chosen
        .iter()
        .map(|c| tidy(c))
        .filter(|c| !c.is_empty())
        .collect();
    if chosen.is_empty() {
        return Ok(Some(vec![]));
    }
    let work = crate::ops::free_name(into, &format!(".{}.extracting", stem(&name)));
    fs::create_dir(&work).map_err(|e| format!("can't make a folder here: {e}"))?;
    let room = (free_space(into) as f64 * 0.95) as u64;
    let mut out = Out {
        root: &work,
        room,
        written: 0,
        skipped: vec![],
    };
    let pick = |n: &str| -> Option<String> {
        let n = tidy(n);
        chosen.iter().find_map(|c| {
            (n == *c || n.starts_with(&format!("{c}/"))).then(|| {
                let parent = c.rsplit_once('/').map(|(p, _)| p.len() + 1).unwrap_or(0);
                n[parent..].to_string()
            })
        })
    };
    let r = unpack(fmt, archive, into, prog, &mut out, &pick);
    let finish = || -> Result<Vec<PathBuf>, String> {
        let mut made = vec![];
        for e in fs::read_dir(&work).map_err(|e| e.to_string())?.flatten() {
            let n = e.file_name().to_string_lossy().into_owned();
            let target = if fs::symlink_metadata(into.join(&n)).is_ok() {
                crate::ops::free_name(into, &n)
            } else {
                into.join(&n)
            };
            fs::rename(e.path(), &target).map_err(|e| e.to_string())?;
            made.push(target);
        }
        Ok(made)
    };
    let result = match r {
        Ok(true) => finish().map(Some),
        Ok(false) => Ok(None),
        Err(e) => Err(e),
    };
    let _ = fs::remove_dir_all(&work);
    result
}

struct Silent;
impl Progress for Silent {
    fn at(&mut self, _: u64, _: u64, _: &str) {}
    fn go_on(&mut self) -> bool {
        true
    }
}

/// One file from inside, as a real file to open or preview: taken out into
/// ~/.cache/whiterose/files/opened (once while the archive doesn't change).
pub fn cached_file(archive: &Path, inside: &str) -> Result<PathBuf, String> {
    use std::hash::{Hash, Hasher};
    let m = fs::metadata(archive).map_err(|e| e.to_string())?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (archive, m.len(), m.modified().ok()).hash(&mut h);
    let base = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| crate::places::home().join(".cache"))
        .join("whiterose/files/opened")
        .join(format!("{:016x}", h.finish()));
    let inside = tidy(inside);
    let file_name = inside.rsplit('/').next().unwrap_or(&inside).to_string();
    let dir = base.join(inside.rsplit_once('/').map(|(p, _)| p).unwrap_or(""));
    let want = dir.join(&file_name);
    if want.is_file() {
        return Ok(want);
    }
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    match extract_some(archive, &[inside], &dir, &mut Silent)? {
        Some(v) if v.first() == Some(&want) => Ok(want),
        Some(v) => v
            .into_iter()
            .next()
            .ok_or_else(|| "it isn't in the archive".into()),
        None => Err("stopped".into()),
    }
}

// ---- making archives -------------------------------------------------------------------------

/// What goes in: each file and folder under `sources`, named from the
/// folder they're in (links kept as links).
fn gather(sources: &[PathBuf]) -> Vec<(PathBuf, String, fs::Metadata)> {
    let mut v = vec![];
    for s in sources {
        let base = s.parent().unwrap_or(Path::new("/"));
        let mut stack = vec![s.clone()];
        while let Some(p) = stack.pop() {
            let Ok(m) = fs::symlink_metadata(&p) else {
                continue;
            };
            let name = p
                .strip_prefix(base)
                .unwrap_or(&p)
                .to_string_lossy()
                .into_owned();
            if m.is_dir() {
                if let Ok(rd) = fs::read_dir(&p) {
                    let mut kids: Vec<PathBuf> = rd.flatten().map(|e| e.path()).collect();
                    kids.sort();
                    stack.extend(kids.into_iter().rev());
                }
            }
            if m.is_dir() || m.is_file() || m.file_type().is_symlink() {
                v.push((p, name, m));
            }
        }
    }
    v
}

/// Splits `file` into `name.001`, `name.002`... of `size` bytes beside it
/// (`file` itself goes). Returns the first part.
fn split(file: &Path, name: &Path, size: u64) -> Result<PathBuf, String> {
    let mut f = fs::File::open(file).map_err(|e| e.to_string())?;
    let mut buf = vec![0u8; 1 << 20];
    let mut first = None;
    for i in 1.. {
        let part = PathBuf::from(format!("{}.{i:03}", name.display()));
        let mut out = fs::File::create(&part).map_err(|e| e.to_string())?;
        let mut left = size;
        let mut any = false;
        while left > 0 {
            let want = buf.len().min(left as usize);
            let n = f.read(&mut buf[..want]).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            out.write_all(&buf[..n]).map_err(|e| e.to_string())?;
            left -= n as u64;
            any = true;
        }
        if !any {
            drop(out);
            let _ = fs::remove_file(&part);
            break;
        }
        first.get_or_insert(part);
        if left > 0 {
            break;
        }
    }
    let _ = fs::remove_file(file);
    first.ok_or_else(|| "nothing to split".into())
}

/// Reading a source file counts towards the progress.
struct Feeding<'a, R> {
    inner: R,
    done: &'a mut u64,
}

impl<R: Read> Read for Feeding<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        *self.done += n as u64;
        Ok(n)
    }
}

/// Makes the archive `dest` (its format from its name) of `sources`, with
/// a password (zip, 7z: AES-256) and split into parts of `split` bytes (0:
/// whole). Written under a hidden name and put in place when complete.
/// Returns the archive (or its first part); None when cancelled.
pub fn compress(
    sources: &[PathBuf],
    dest: &Path,
    password: Option<&str>,
    split_at: u64,
    prog: &mut dyn Progress,
) -> Result<Option<PathBuf>, String> {
    let name = dest
        .file_name()
        .ok_or("no name")?
        .to_string_lossy()
        .into_owned();
    let fmt = format_of(&name).ok_or("not a kind of archive Files makes")?;
    let password = password.filter(|p| !p.is_empty());
    if password.is_some() && !matches!(fmt, Format::Zip | Format::SevenZ) {
        return Err("only zip and 7z archives can have a password".into());
    }
    let dir = dest.parent().ok_or("no folder")?;
    let work = dir.join(format!(".{name}.wrpart"));
    let items = gather(sources);
    let total: u64 = items
        .iter()
        .filter(|(_, _, m)| m.is_file())
        .map(|(_, _, m)| m.len())
        .sum();
    let mut done = 0u64;
    let r = (|| -> Result<bool, String> {
        let out = fs::File::create(&work).map_err(|e| e.to_string())?;
        match fmt {
            Format::Zip => {
                use zip::write::SimpleFileOptions;
                let mut z = zip::ZipWriter::new(out);
                for (p, n, m) in &items {
                    if !prog.go_on() {
                        return Ok(false);
                    }
                    prog.at(done, total, n);
                    let mut o = SimpleFileOptions::default()
                        .compression_method(zip::CompressionMethod::Deflated)
                        .unix_permissions(m.mode() & 0o777)
                        .large_file(m.len() >= 1 << 32);
                    if let Some(pw) = password {
                        o = o.with_aes_encryption(zip::AesMode::Aes256, pw);
                    }
                    if m.file_type().is_symlink() {
                        let t = fs::read_link(p).map_err(|e| e.to_string())?;
                        z.add_symlink(n, t.to_string_lossy(), o)
                            .map_err(|e| e.to_string())?;
                    } else if m.is_dir() {
                        z.add_directory(n, o).map_err(|e| e.to_string())?;
                    } else {
                        z.start_file(n, o).map_err(|e| e.to_string())?;
                        let mut f = Feeding {
                            inner: fs::File::open(p).map_err(|e| e.to_string())?,
                            done: &mut done,
                        };
                        io::copy(&mut f, &mut z).map_err(|e| e.to_string())?;
                    }
                }
                z.finish().map_err(|e| e.to_string())?;
            }
            Format::SevenZ => {
                let mut z = sevenz_rust::SevenZWriter::new(out).map_err(|e| e.to_string())?;
                if let Some(pw) = password {
                    z.set_content_methods(vec![
                        sevenz_rust::AesEncoderOptions::new(sevenz_rust::Password::from(pw)).into(),
                        sevenz_rust::SevenZMethod::LZMA2.into(),
                    ]);
                }
                for (p, n, m) in &items {
                    if !prog.go_on() {
                        return Ok(false);
                    }
                    prog.at(done, total, n);
                    let entry = sevenz_rust::SevenZArchiveEntry::from_path(p, n.clone());
                    if m.is_file() {
                        let f = Feeding {
                            inner: fs::File::open(p).map_err(|e| e.to_string())?,
                            done: &mut done,
                        };
                        z.push_archive_entry(entry, Some(f))
                            .map_err(|e| e.to_string())?;
                    } else if m.is_dir() {
                        z.push_archive_entry::<fs::File>(entry, None)
                            .map_err(|e| e.to_string())?;
                    }
                    // (7z keeps no links: they're left out.)
                }
                z.finish().map_err(|e| e.to_string())?;
            }
            tar_fmt => {
                let w: Box<dyn Write> = match tar_fmt {
                    Format::TarGz => Box::new(flate2::write::GzEncoder::new(
                        out,
                        flate2::Compression::default(),
                    )),
                    Format::TarXz => Box::new(xz2::write::XzEncoder::new(out, 6)),
                    Format::TarBz2 => Box::new(bzip2::write::BzEncoder::new(
                        out,
                        bzip2::Compression::default(),
                    )),
                    Format::TarZst => Box::new(
                        zstd::stream::write::Encoder::new(out, 3)
                            .map_err(|e| e.to_string())?
                            .auto_finish(),
                    ),
                    _ => Box::new(out),
                };
                let mut t = tar::Builder::new(w);
                t.follow_symlinks(false);
                for (p, n, m) in &items {
                    if !prog.go_on() {
                        return Ok(false);
                    }
                    prog.at(done, total, n);
                    if m.is_file() {
                        let mut h = tar::Header::new_gnu();
                        h.set_metadata(m);
                        let f = Feeding {
                            inner: fs::File::open(p).map_err(|e| e.to_string())?,
                            done: &mut done,
                        };
                        t.append_data(&mut h, n, f).map_err(|e| e.to_string())?;
                    } else {
                        t.append_path_with_name(p, n).map_err(|e| e.to_string())?;
                    }
                }
                t.into_inner()
                    .map_err(|e| e.to_string())?
                    .flush()
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(true)
    })();
    match r {
        Ok(true) => {}
        Ok(false) => {
            let _ = fs::remove_file(&work);
            return Ok(None);
        }
        Err(e) => {
            let _ = fs::remove_file(&work);
            return Err(e);
        }
    }
    prog.at(total, total, "");
    let size = fs::metadata(&work).map(|m| m.len()).unwrap_or(0);
    if split_at > 0 && size > split_at {
        return split(&work, dest, split_at).map(Some);
    }
    fs::rename(&work, dest).map_err(|e| e.to_string())?;
    Ok(Some(dest.to_path_buf()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::MetadataExt;

    #[test]
    fn make_then_extract_every_format_with_passwords_and_parts() {
        let d = scratch("roundtrip");
        let src = d.join("garden");
        fs::create_dir_all(src.join("beds/empty")).unwrap();
        fs::write(src.join("plan.txt"), "beans\n").unwrap();
        // (Doesn't compress: the split test needs it bigger than a part.)
        let mut x = 12345u32;
        let big: Vec<u8> = (0..300_000)
            .map(|_| {
                x = x.wrapping_mul(1_103_515_245).wrapping_add(12_345);
                (x >> 16) as u8
            })
            .collect();
        fs::write(src.join("beds/photo.bin"), &big).unwrap();
        let check = |made: &Path| {
            assert_eq!(
                fs::read_to_string(made.join("plan.txt")).unwrap(),
                "beans\n"
            );
            assert_eq!(fs::read(made.join("beds/photo.bin")).unwrap(), big);
            assert!(made.join("beds/empty").is_dir());
        };
        for (i, name) in [
            "g.zip",
            "g.7z",
            "g.tar.gz",
            "g.tar.xz",
            "g.tar.zst",
            "g.tar.bz2",
        ]
        .iter()
        .enumerate()
        {
            let out = d.join(format!("out{i}"));
            fs::create_dir_all(&out).unwrap();
            let a = compress(
                std::slice::from_ref(&src),
                &out.join(name),
                None,
                0,
                &mut Quiet(0, None),
            )
            .unwrap()
            .unwrap();
            let (made, skipped) = extract(&a, None, &mut Quiet(0, None)).unwrap().unwrap();
            assert!(skipped.is_empty(), "{name}: {skipped:?}");
            check(&made);
            assert!(!out.join(format!(".{name}.wrpart")).exists());
        }
        // A password: asked for, refused when wrong, used when right.
        for name in ["locked.zip", "locked.7z"] {
            let out = d.join(name.replace('.', "-"));
            fs::create_dir_all(&out).unwrap();
            let a = compress(
                std::slice::from_ref(&src),
                &out.join(name),
                Some("hunter2"),
                0,
                &mut Quiet(0, None),
            )
            .unwrap()
            .unwrap();
            assert_eq!(
                extract(&a, None, &mut Quiet(0, None)).unwrap_err(),
                NEEDS_PASSWORD,
                "{name}"
            );
            set_password(&a, "wrong");
            assert_eq!(
                extract(&a, None, &mut Quiet(0, None)).unwrap_err(),
                WRONG_PASSWORD,
                "{name}"
            );
            set_password(&a, "hunter2");
            check(&extract(&a, None, &mut Quiet(0, None)).unwrap().unwrap().0);
        }
        assert!(compress(
            std::slice::from_ref(&src),
            &d.join("x.tar.gz"),
            Some("pw"),
            0,
            &mut Quiet(0, None)
        )
        .is_err());
        // Split into parts, extracted from the first.
        let out = d.join("parts");
        fs::create_dir_all(&out).unwrap();
        let first = compress(
            std::slice::from_ref(&src),
            &out.join("g.zip"),
            None,
            100_000,
            &mut Quiet(0, None),
        )
        .unwrap()
        .unwrap();
        assert!(first.ends_with("g.zip.001"));
        assert!(out.join("g.zip.002").exists() && !out.join("g.zip").exists());
        assert_eq!(format_of("g.zip.001"), Some(Format::Zip));
        check(
            &extract(&first, None, &mut Quiet(0, None))
                .unwrap()
                .unwrap()
                .0,
        );
        // Cancelled: nothing left behind.
        assert!(
            compress(&[src], &out.join("c.zip"), None, 0, &mut Quiet(0, Some(2)))
                .unwrap()
                .is_none()
        );
        assert!(!out.join("c.zip").exists() && !out.join(".c.zip.wrpart").exists());
        fs::remove_dir_all(&d).unwrap();
    }

    #[test]
    fn looking_inside_and_taking_out() {
        assert_eq!(
            split_place("archive:/a b/x.zip!/docs/2024/"),
            Some((PathBuf::from("/a b/x.zip"), "docs/2024".into()))
        );
        assert_eq!(
            split_place("archive:/x.zip!/"),
            Some((PathBuf::from("/x.zip"), String::new()))
        );
        assert_eq!(place(Path::new("/x.zip"), "docs/"), "archive:/x.zip!/docs");
        let d = scratch("inside");
        let src = d.join("trip");
        fs::create_dir_all(src.join("photos/day1")).unwrap();
        fs::write(src.join("notes.txt"), "ferry 09:40").unwrap();
        fs::write(src.join("photos/day1/a.jpg"), "jpeg").unwrap();
        fs::write(src.join("photos/b.jpg"), "jpeg2").unwrap();
        for name in ["t.zip", "t.7z", "t.tar.zst"] {
            let a = compress(
                std::slice::from_ref(&src),
                &d.join(name),
                None,
                0,
                &mut Quiet(0, None),
            )
            .unwrap()
            .unwrap();
            let names = |v: Vec<Item>| {
                let mut n: Vec<String> = v
                    .into_iter()
                    .map(|i| format!("{}{}", i.path, if i.dir { "/" } else { "" }))
                    .collect();
                n.sort();
                n
            };
            assert_eq!(names(list(&a, "").unwrap()), vec!["trip/"], "{name}");
            assert_eq!(
                names(list(&a, "trip").unwrap()),
                vec!["trip/notes.txt", "trip/photos/"],
                "{name}"
            );
            assert_eq!(
                names(list(&a, "trip/photos/").unwrap()),
                vec!["trip/photos/b.jpg", "trip/photos/day1/"],
                "{name}"
            );
            // A folder and a file out, twice: the second lot gets numbers.
            let out = d.join(format!("out-{name}"));
            fs::create_dir_all(&out).unwrap();
            let chosen = ["trip/photos".to_string(), "trip/notes.txt".to_string()];
            let made = extract_some(&a, &chosen, &out, &mut Quiet(0, None))
                .unwrap()
                .unwrap();
            assert_eq!(made.len(), 2, "{name}");
            assert_eq!(
                fs::read_to_string(out.join("photos/day1/a.jpg")).unwrap(),
                "jpeg"
            );
            assert_eq!(
                fs::read_to_string(out.join("notes.txt")).unwrap(),
                "ferry 09:40"
            );
            extract_some(&a, &chosen, &out, &mut Quiet(0, None))
                .unwrap()
                .unwrap();
            assert!(
                out.join("notes (2).txt").is_file() && out.join("photos (2)/b.jpg").is_file(),
                "{:?}",
                fs::read_dir(&out)
                    .unwrap()
                    .map(|e| e.unwrap().file_name())
                    .collect::<Vec<_>>()
            );
            assert!(
                !fs::read_dir(&out).unwrap().any(|e| e
                    .unwrap()
                    .file_name()
                    .to_string_lossy()
                    .starts_with('.')),
                "no work folder left"
            );
            // One file as a real file (cached).
            std::env::set_var("XDG_CACHE_HOME", d.join("cache"));
            let f = cached_file(&a, "trip/photos/b.jpg").unwrap();
            assert_eq!(fs::read_to_string(&f).unwrap(), "jpeg2");
            assert_eq!(cached_file(&a, "trip/photos/b.jpg").unwrap(), f);
        }
        fs::remove_dir_all(&d).unwrap();
    }

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
