//! File operations: copy, move, trash, delete, rename and new folder.
//!
//! The long ones run on a worker thread through `run`, telling a `Report`
//! how far they've got and asking it what to do when something's already
//! there (replace, keep both, skip; "for all" remembered). They stop between
//! files, or between chunks of a big file, when cancelled or paused.
//!
//! Copies write `.name.wrpart` next to the target and rename it into place
//! when complete, so a replaced file is never half-written, and a cancelled
//! copy leaves nothing behind for the file it was on.

use std::{
    fs,
    io::{Read, Write},
    os::unix::fs::{symlink, MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::Duration,
};

use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Copy,
    Move,
    Trash,
    Delete,
    /// Deleting things in the Trash for good (their Trash records too).
    Purge,
    /// Archives unpacked, each into a new folder (archive.rs).
    Extract,
}

impl Kind {
    pub fn parse(s: &str) -> Option<Kind> {
        Some(match s {
            "copy" => Kind::Copy,
            "move" => Kind::Move,
            "trash" => Kind::Trash,
            "delete" => Kind::Delete,
            "purge" | "empty" => Kind::Purge,
            "extract" => Kind::Extract,
            _ => return None,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Choice {
    Replace,
    KeepBoth,
    Skip,
}

impl Choice {
    pub fn parse(s: &str) -> Choice {
        match s {
            "replace" => Choice::Replace,
            "keep" => Choice::KeepBoth,
            _ => Choice::Skip,
        }
    }
}

#[derive(Default)]
pub struct Control {
    pub cancel: AtomicBool,
    pub pause: AtomicBool,
}

impl Control {
    /// Waits out a pause; false once cancelled.
    fn go_on(&self) -> bool {
        while self.pause.load(Ordering::Relaxed) && !self.cancel.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(80));
        }
        !self.cancel.load(Ordering::Relaxed)
    }
}

#[derive(Default, Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    pub bytes_done: u64,
    pub bytes_total: u64,
    pub items_done: u64,
    pub items_total: u64,
    pub current: String,
}

pub trait Report {
    fn progress(&mut self, p: &Progress);
    /// `src` would land on `dst`, which is there already. (choice, for all).
    fn conflict(&mut self, src: &Path, dst: &Path) -> (Choice, bool);
}

#[derive(Default, Debug, Serialize)]
pub struct Outcome {
    /// Where each top-level item ended up.
    pub done: Vec<String>,
    /// [path, what went wrong]
    pub failed: Vec<(String, String)>,
    pub cancelled: bool,
    /// [from, to] for what moved (to the Trash too): folders' looks follow.
    #[serde(skip)]
    pub moved: Vec<(String, String)>,
    /// What's gone for good.
    #[serde(skip)]
    pub gone: Vec<String>,
}

/// "name (n).ext"; ".tar.gz" and friends stay together, dotfiles have no ext.
pub fn numbered(name: &str, n: u32) -> String {
    let split = name
        .find(".tar.")
        .filter(|&i| i > 0)
        .or_else(|| name.rfind('.').filter(|&i| i > 0));
    match split {
        Some(i) => format!("{} ({n}){}", &name[..i], &name[i..]),
        None => format!("{name} ({n})"),
    }
}

/// A name in `dir` like `name` that isn't taken.
pub fn free_name(dir: &Path, name: &str) -> PathBuf {
    let mut n = 2;
    loop {
        let p = dir.join(numbered(name, n));
        if fs::symlink_metadata(&p).is_err() {
            return p;
        }
        n += 1;
    }
}

fn friendly(e: std::io::Error) -> String {
    match e.raw_os_error() {
        Some(libc::ENOSPC) => "the disk is full".into(),
        Some(libc::EACCES) | Some(libc::EPERM) => "permission denied".into(),
        Some(libc::EROFS) => "the drive is read-only".into(),
        Some(libc::ENAMETOOLONG) => "the name is too long".into(),
        Some(libc::ENOENT) => "it's not there any more".into(),
        _ => e.to_string(),
    }
}

/// A file or folder name someone typed, checked.
pub fn check_name(name: &str) -> Result<&str, String> {
    let n = name.trim();
    if n.is_empty() {
        return Err("a name is needed".into());
    }
    if n == "." || n == ".." || n.contains('/') || n.contains('\0') {
        return Err("names can't contain /".into());
    }
    if n.len() > 255 {
        return Err("the name is too long".into());
    }
    Ok(n)
}

pub fn rename(path: &Path, name: &str) -> Result<PathBuf, String> {
    let name = check_name(name)?;
    let parent = path.parent().ok_or("can't rename this")?;
    let target = parent.join(name);
    if target == path {
        return Ok(target);
    }
    // Only a change of case on a case-insensitive drive may land on itself.
    if let (Ok(a), Ok(b)) = (fs::symlink_metadata(path), fs::symlink_metadata(&target)) {
        if a.ino() != b.ino() || a.dev() != b.dev() {
            return Err(format!("“{name}” already exists here"));
        }
    }
    fs::rename(path, &target).map_err(friendly)?;
    Ok(target)
}

/// A new folder (`file`: an empty file) in `dir`, numbered if the name's taken.
pub fn create(dir: &Path, name: &str, file: bool) -> Result<PathBuf, String> {
    let name = check_name(name)?;
    let mut target = dir.join(name);
    if fs::symlink_metadata(&target).is_ok() {
        target = free_name(dir, name);
    }
    if file {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&target)
            .map_err(friendly)?;
    } else {
        fs::create_dir(&target).map_err(friendly)?;
    }
    Ok(target)
}

/// Bytes and items under `p` (not following links).
fn measure(p: &Path, bytes: &mut u64, items: &mut u64) {
    let Ok(m) = fs::symlink_metadata(p) else {
        return;
    };
    *items += 1;
    if m.is_dir() {
        if let Ok(rd) = fs::read_dir(p) {
            for e in rd.flatten() {
                measure(&e.path(), bytes, items);
            }
        }
    } else {
        *bytes += m.len();
    }
}

struct Run<'a, R: Report> {
    report: &'a mut R,
    ctl: &'a Control,
    prog: Progress,
    all: Option<Choice>,
    out: Outcome,
}

impl<'a, R: Report> Run<'a, R> {
    fn tell(&mut self, current: &Path) {
        self.prog.current = current
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        self.report.progress(&self.prog);
    }

    fn fail(&mut self, p: &Path, why: String) {
        self.out
            .failed
            .push((p.to_string_lossy().into_owned(), why));
    }

    fn ask(&mut self, src: &Path, dst: &Path) -> Choice {
        if let Some(c) = self.all {
            return c;
        }
        let (c, all) = self.report.conflict(src, dst);
        if all {
            self.all = Some(c);
        }
        c
    }

    /// Where `src` goes in `dir`, after asking if something's there
    /// (None: skip it). A folder onto a folder merges.
    fn place(&mut self, src: &Path, dir: &Path) -> Option<PathBuf> {
        let name = src.file_name()?;
        let target = dir.join(name);
        if target == src {
            // Copying into its own folder: a numbered copy.
            return Some(free_name(dir, &name.to_string_lossy()));
        }
        let Ok(there) = fs::symlink_metadata(&target) else {
            return Some(target);
        };
        match self.ask(src, &target) {
            Choice::Replace => {
                let src_dir = fs::symlink_metadata(src)
                    .map(|m| m.is_dir())
                    .unwrap_or(false);
                // Folder onto folder: merge; anything else: the old one goes
                // once the new one is ready (files), or now (folders).
                if there.is_dir() && !src_dir {
                    if let Err(e) = fs::remove_dir_all(&target) {
                        self.fail(&target, friendly(e));
                        return None;
                    }
                } else if !there.is_dir() && src_dir {
                    if let Err(e) = fs::remove_file(&target) {
                        self.fail(&target, friendly(e));
                        return None;
                    }
                }
                Some(target)
            }
            Choice::KeepBoth => Some(free_name(dir, &name.to_string_lossy())),
            Choice::Skip => {
                let (mut b, mut i) = (0, 0);
                measure(src, &mut b, &mut i);
                self.prog.bytes_done += b;
                self.prog.items_done += i;
                None
            }
        }
    }

    /// Copies `src` to `target` (folders recursively). False once cancelled.
    fn copy(&mut self, src: &Path, target: &Path) -> bool {
        if !self.ctl.go_on() {
            return false;
        }
        let meta = match fs::symlink_metadata(src) {
            Ok(m) => m,
            Err(e) => {
                self.fail(src, friendly(e));
                return true;
            }
        };
        self.tell(src);
        if meta.file_type().is_symlink() {
            let r = fs::read_link(src).and_then(|to| {
                if fs::symlink_metadata(target).is_ok() {
                    fs::remove_file(target)?;
                }
                symlink(to, target)
            });
            if let Err(e) = r {
                self.fail(src, friendly(e));
            }
            self.prog.bytes_done += meta.len();
            self.prog.items_done += 1;
        } else if meta.is_dir() {
            if fs::symlink_metadata(target).is_err() {
                if let Err(e) = fs::create_dir(target) {
                    self.fail(src, friendly(e));
                    return true;
                }
            }
            self.prog.items_done += 1;
            let children = match fs::read_dir(src) {
                Ok(rd) => rd.flatten().map(|e| e.path()).collect::<Vec<_>>(),
                Err(e) => {
                    self.fail(src, friendly(e));
                    return true;
                }
            };
            for child in children {
                let Some(t) = self.place(&child, target) else {
                    continue;
                };
                if !self.copy(&child, &t) {
                    return false;
                }
            }
            let _ = fs::set_permissions(target, meta.permissions());
            crate::tags::copy(src, target);
        } else if meta.is_file() {
            match self.copy_file(src, target, &meta) {
                Ok(true) => {}
                Ok(false) => return false,
                Err(e) => self.fail(src, e),
            }
            self.prog.items_done += 1;
        } else {
            self.fail(src, "devices, sockets and pipes aren't copied".into());
            self.prog.items_done += 1;
        }
        self.report.progress(&self.prog);
        true
    }

    fn copy_file(
        &mut self,
        src: &Path,
        target: &Path,
        meta: &fs::Metadata,
    ) -> Result<bool, String> {
        let dir = target.parent().ok_or("no folder")?;
        let part = dir.join(format!(
            ".{}.wrpart",
            target.file_name().unwrap_or_default().to_string_lossy()
        ));
        let mut from = fs::File::open(src).map_err(friendly)?;
        let mut to = fs::File::create(&part).map_err(friendly)?;
        let mut buf = vec![0u8; 1 << 20];
        let result = (|| loop {
            if !self.ctl.go_on() {
                return Ok(false);
            }
            let n = from.read(&mut buf).map_err(friendly)?;
            if n == 0 {
                return Ok(true);
            }
            to.write_all(&buf[..n]).map_err(friendly)?;
            self.prog.bytes_done += n as u64;
            self.report.progress(&self.prog);
        })();
        drop(to);
        match result {
            Ok(true) => {
                let _ =
                    fs::set_permissions(&part, fs::Permissions::from_mode(meta.mode() & 0o7777));
                keep_times(&part, meta);
                // Tags go with the copy.
                crate::tags::copy(src, &part);
                fs::rename(&part, target).map_err(|e| {
                    let _ = fs::remove_file(&part);
                    friendly(e)
                })?;
                Ok(true)
            }
            other => {
                let _ = fs::remove_file(&part);
                other
            }
        }
    }

    /// Moves `src` to `target`: a rename on the same drive (merging folders
    /// one entry at a time), otherwise a copy and then removing the original.
    fn mv(&mut self, src: &Path, target: &Path) -> bool {
        if !self.ctl.go_on() {
            return false;
        }
        self.tell(src);
        let (mut b, mut i) = (0, 0);
        measure(src, &mut b, &mut i);
        let merging = fs::symlink_metadata(target)
            .map(|m| m.is_dir())
            .unwrap_or(false)
            && fs::symlink_metadata(src)
                .map(|m| m.is_dir())
                .unwrap_or(false);
        if !merging {
            match fs::rename(src, target) {
                Ok(()) => {
                    self.prog.bytes_done += b;
                    self.prog.items_done += i;
                    self.report.progress(&self.prog);
                    return true;
                }
                Err(e) if e.raw_os_error() == Some(libc::EXDEV) => {
                    let failed = self.out.failed.len();
                    if !self.copy(src, target) {
                        return false;
                    }
                    // Only a complete copy lets the original go.
                    if self.out.failed.len() == failed {
                        let gone = if fs::symlink_metadata(src)
                            .map(|m| m.is_dir())
                            .unwrap_or(false)
                        {
                            fs::remove_dir_all(src)
                        } else {
                            fs::remove_file(src)
                        };
                        if let Err(e) = gone {
                            self.fail(
                                src,
                                format!("copied, but the original stayed: {}", friendly(e)),
                            );
                        }
                    }
                    return true;
                }
                Err(e) => {
                    self.fail(src, friendly(e));
                    return true;
                }
            }
        }
        // Folder into an existing folder: each entry, asking about clashes.
        self.prog.items_done += 1;
        let children = match fs::read_dir(src) {
            Ok(rd) => rd.flatten().map(|e| e.path()).collect::<Vec<_>>(),
            Err(e) => {
                self.fail(src, friendly(e));
                return true;
            }
        };
        for child in children {
            let Some(t) = self.place(&child, target) else {
                continue;
            };
            if !self.mv(&child, &t) {
                return false;
            }
        }
        // Left behind if anything inside was skipped.
        let _ = fs::remove_dir(src);
        true
    }
}

fn keep_times(p: &Path, meta: &fs::Metadata) {
    use std::os::unix::ffi::OsStrExt;
    let Ok(c) = std::ffi::CString::new(p.as_os_str().as_bytes()) else {
        return;
    };
    let times = [
        libc::timespec {
            tv_sec: meta.atime(),
            tv_nsec: meta.atime_nsec(),
        },
        libc::timespec {
            tv_sec: meta.mtime(),
            tv_nsec: meta.mtime_nsec(),
        },
    ];
    // SAFETY: a valid C path and two timespecs.
    unsafe {
        libc::utimensat(libc::AT_FDCWD, c.as_ptr(), times.as_ptr(), 0);
    }
}

/// Runs one operation over `sources` (absolute paths); `dest` is the folder
/// for copy and move.
pub fn run<R: Report>(
    kind: Kind,
    sources: &[PathBuf],
    dest: Option<&Path>,
    ctl: &Control,
    report: &mut R,
) -> Outcome {
    let mut prog = Progress::default();
    for s in sources {
        if kind == Kind::Extract {
            // (The archives themselves: what's in them isn't known yet.)
            prog.bytes_total += fs::metadata(s).map(|m| m.len()).unwrap_or(0);
            prog.items_total += 1;
        } else {
            measure(s, &mut prog.bytes_total, &mut prog.items_total);
        }
    }
    let mut run = Run {
        report,
        ctl,
        prog,
        all: None,
        out: Outcome::default(),
    };
    run.report.progress(&run.prog);
    for src in sources {
        if !ctl.go_on() {
            run.out.cancelled = true;
            break;
        }
        match kind {
            Kind::Copy | Kind::Move => {
                let Some(dir) = dest else {
                    run.fail(src, "nowhere to put it".into());
                    continue;
                };
                if dir.starts_with(src)
                    && (kind == Kind::Move || dir != src.parent().unwrap_or(dir))
                {
                    run.fail(src, "a folder can't go inside itself".into());
                    continue;
                }
                if kind == Kind::Move && src.parent() == Some(dir) {
                    // Already here.
                    run.out.done.push(src.to_string_lossy().into_owned());
                    continue;
                }
                let Some(target) = run.place(src, dir) else {
                    continue;
                };
                let ok = if kind == Kind::Copy {
                    run.copy(src, &target)
                } else {
                    run.mv(src, &target)
                };
                if !ok {
                    run.out.cancelled = true;
                    break;
                }
                if kind == Kind::Move && fs::symlink_metadata(src).is_err() {
                    run.out.moved.push((
                        src.to_string_lossy().into_owned(),
                        target.to_string_lossy().into_owned(),
                    ));
                }
                run.out.done.push(target.to_string_lossy().into_owned());
            }
            Kind::Trash => {
                run.tell(src);
                let (mut b, mut i) = (0, 0);
                measure(src, &mut b, &mut i);
                match crate::trash::put(src) {
                    Ok(p) => {
                        run.out.moved.push((
                            src.to_string_lossy().into_owned(),
                            p.to_string_lossy().into_owned(),
                        ));
                        run.out.done.push(p.to_string_lossy().into_owned());
                    }
                    Err(e) => run.fail(src, e),
                }
                run.prog.bytes_done += b;
                run.prog.items_done += i;
            }
            Kind::Delete | Kind::Purge => {
                run.tell(src);
                let info = (kind == Kind::Purge)
                    .then(|| crate::trash::info_of(src))
                    .flatten();
                if !delete(&mut run, src) {
                    run.out.cancelled = true;
                    break;
                }
                if fs::symlink_metadata(src).is_err() {
                    run.out.gone.push(src.to_string_lossy().into_owned());
                    if let Some(i) = info {
                        let _ = fs::remove_file(i);
                    }
                }
            }
            Kind::Extract => {
                run.tell(src);
                // Progress is the archive's bytes read; it starts from what's
                // been done with the archives before it.
                let before = run.prog.bytes_done;
                struct Along<'b, 'a, R: Report> {
                    run: &'b mut Run<'a, R>,
                    before: u64,
                }
                impl<R: Report> crate::archive::Progress for Along<'_, '_, R> {
                    fn at(&mut self, done: u64, _total: u64, entry: &str) {
                        self.run.prog.bytes_done = self.before + done;
                        self.run.prog.current = entry
                            .rsplit('/')
                            .find(|p| !p.is_empty())
                            .unwrap_or(entry)
                            .to_string();
                        self.run.report.progress(&self.run.prog);
                    }
                    fn go_on(&mut self) -> bool {
                        self.run.ctl.go_on()
                    }
                }
                let size = fs::metadata(src).map(|m| m.len()).unwrap_or(0);
                let mut along = Along {
                    run: &mut run,
                    before,
                };
                match crate::archive::extract(src, dest, &mut along) {
                    Ok(Some((made, skipped))) => {
                        for (e, why) in skipped {
                            run.fail(&made.join(&e), why);
                        }
                        run.out.done.push(made.to_string_lossy().into_owned());
                    }
                    Ok(None) => {
                        run.out.cancelled = true;
                        break;
                    }
                    Err(e) => run.fail(src, e),
                }
                run.prog.bytes_done = before + size;
                run.prog.items_done += 1;
            }
        }
    }
    run.report.progress(&run.prog);
    run.out
}

fn delete<R: Report>(run: &mut Run<R>, p: &Path) -> bool {
    if !run.ctl.go_on() {
        return false;
    }
    let Ok(m) = fs::symlink_metadata(p) else {
        return true;
    };
    if m.is_dir() {
        if let Ok(rd) = fs::read_dir(p) {
            for e in rd.flatten() {
                if !delete(run, &e.path()) {
                    return false;
                }
            }
        }
        if let Err(e) = fs::remove_dir(p) {
            run.fail(p, friendly(e));
        }
    } else {
        if let Err(e) = fs::remove_file(p) {
            run.fail(p, friendly(e));
        }
        run.prog.bytes_done += m.len();
    }
    run.prog.items_done += 1;
    run.report.progress(&run.prog);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("wr-ops-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    /// Answers every conflict the same way, counting them.
    struct Auto {
        choice: Choice,
        all: bool,
        asked: u32,
        last: Progress,
        cancel_at: Option<u64>,
        ctl: Option<std::sync::Arc<Control>>,
    }
    impl Auto {
        fn new(choice: Choice) -> Self {
            Auto {
                choice,
                all: false,
                asked: 0,
                last: Progress::default(),
                cancel_at: None,
                ctl: None,
            }
        }
    }
    impl Report for Auto {
        fn progress(&mut self, p: &Progress) {
            self.last = p.clone();
            if let (Some(at), Some(c)) = (self.cancel_at, &self.ctl) {
                if p.bytes_done >= at {
                    c.cancel.store(true, Ordering::Relaxed);
                }
            }
        }
        fn conflict(&mut self, _: &Path, _: &Path) -> (Choice, bool) {
            self.asked += 1;
            (self.choice, self.all)
        }
    }

    fn tree(root: &Path) {
        fs::create_dir_all(root.join("src/sub")).unwrap();
        fs::write(root.join("src/a.txt"), "aaaa").unwrap();
        fs::write(root.join("src/sub/b.txt"), "bb").unwrap();
        symlink("a.txt", root.join("src/link")).unwrap();
        fs::set_permissions(root.join("src/a.txt"), fs::Permissions::from_mode(0o640)).unwrap();
    }

    #[test]
    fn names() {
        assert_eq!(numbered("notes.txt", 2), "notes (2).txt");
        assert_eq!(numbered("photos", 3), "photos (3)");
        assert_eq!(numbered("backup.tar.gz", 2), "backup (2).tar.gz");
        assert_eq!(numbered(".bashrc", 2), ".bashrc (2)");
        assert!(check_name("  ").is_err());
        assert!(check_name("a/b").is_err());
        assert!(check_name("..").is_err());
        assert_eq!(check_name(" ok.txt ").unwrap(), "ok.txt");
    }

    #[test]
    fn copies_a_tree_with_links_modes_and_times() {
        let d = scratch("copy");
        tree(&d);
        fs::create_dir(d.join("dest")).unwrap();
        let ctl = Control::default();
        let mut r = Auto::new(Choice::Skip);
        let out = run(
            Kind::Copy,
            &[d.join("src")],
            Some(&d.join("dest")),
            &ctl,
            &mut r,
        );
        assert!(out.failed.is_empty(), "{:?}", out.failed);
        assert_eq!(
            out.done,
            vec![d.join("dest/src").to_string_lossy().into_owned()]
        );
        assert_eq!(
            fs::read_to_string(d.join("dest/src/sub/b.txt")).unwrap(),
            "bb"
        );
        assert_eq!(
            fs::read_link(d.join("dest/src/link")).unwrap(),
            PathBuf::from("a.txt")
        );
        let m = fs::metadata(d.join("dest/src/a.txt")).unwrap();
        assert_eq!(m.mode() & 0o777, 0o640);
        assert_eq!(
            m.mtime(),
            fs::metadata(d.join("src/a.txt")).unwrap().mtime()
        );
        assert_eq!(
            (r.last.bytes_done, r.last.bytes_total),
            (11, 11),
            "4 + 2 bytes, and the link's 5"
        );
        assert_eq!(r.last.items_done, r.last.items_total);
        assert!(d.join("src/a.txt").exists(), "a copy leaves the original");
        assert_eq!(
            fs::read_dir(d.join("dest/src"))
                .unwrap()
                .flatten()
                .filter(|e| e.file_name().to_string_lossy().ends_with(".wrpart"))
                .count(),
            0
        );
    }

    #[test]
    fn copy_into_the_same_folder_numbers_it() {
        let d = scratch("dup");
        fs::write(d.join("x.txt"), "1").unwrap();
        let mut r = Auto::new(Choice::Skip);
        let out = run(
            Kind::Copy,
            &[d.join("x.txt")],
            Some(&d),
            &Control::default(),
            &mut r,
        );
        assert_eq!(
            out.done,
            vec![d.join("x (2).txt").to_string_lossy().into_owned()]
        );
        assert_eq!(r.asked, 0);
    }

    #[test]
    fn conflicts_replace_keep_skip_and_for_all() {
        let d = scratch("conflict");
        fs::create_dir_all(d.join("from")).unwrap();
        fs::create_dir_all(d.join("to")).unwrap();
        for n in ["1.txt", "2.txt"] {
            fs::write(d.join("from").join(n), "new").unwrap();
            fs::write(d.join("to").join(n), "old").unwrap();
        }
        let srcs = [d.join("from/1.txt"), d.join("from/2.txt")];

        let mut r = Auto::new(Choice::Skip);
        run(
            Kind::Copy,
            &srcs,
            Some(&d.join("to")),
            &Control::default(),
            &mut r,
        );
        assert_eq!(r.asked, 2);
        assert_eq!(fs::read_to_string(d.join("to/1.txt")).unwrap(), "old");

        let mut r = Auto::new(Choice::KeepBoth);
        r.all = true;
        run(
            Kind::Copy,
            &srcs,
            Some(&d.join("to")),
            &Control::default(),
            &mut r,
        );
        assert_eq!(r.asked, 1, "for all: asked once");
        assert_eq!(fs::read_to_string(d.join("to/2 (2).txt")).unwrap(), "new");

        let mut r = Auto::new(Choice::Replace);
        run(
            Kind::Copy,
            &srcs,
            Some(&d.join("to")),
            &Control::default(),
            &mut r,
        );
        assert_eq!(fs::read_to_string(d.join("to/1.txt")).unwrap(), "new");
    }

    #[test]
    fn folder_onto_folder_merges() {
        let d = scratch("merge");
        fs::create_dir_all(d.join("a/photos")).unwrap();
        fs::create_dir_all(d.join("b/photos")).unwrap();
        fs::write(d.join("a/photos/new.jpg"), "n").unwrap();
        fs::write(d.join("b/photos/old.jpg"), "o").unwrap();
        let mut r = Auto::new(Choice::Replace);
        let out = run(
            Kind::Move,
            &[d.join("a/photos")],
            Some(&d.join("b")),
            &Control::default(),
            &mut r,
        );
        assert!(out.failed.is_empty(), "{:?}", out.failed);
        assert!(d.join("b/photos/new.jpg").exists() && d.join("b/photos/old.jpg").exists());
        assert!(
            !d.join("a/photos").exists(),
            "the emptied source folder goes"
        );
    }

    #[test]
    fn move_renames_and_refuses_to_go_inside_itself() {
        let d = scratch("move");
        tree(&d);
        let ctl = Control::default();
        let mut r = Auto::new(Choice::Skip);
        let out = run(
            Kind::Move,
            &[d.join("src")],
            Some(&d.join("src/sub")),
            &ctl,
            &mut r,
        );
        assert_eq!(out.failed.len(), 1);
        assert!(d.join("src").exists());
        fs::create_dir(d.join("dest")).unwrap();
        let out = run(
            Kind::Move,
            &[d.join("src")],
            Some(&d.join("dest")),
            &ctl,
            &mut r,
        );
        assert!(out.failed.is_empty());
        assert!(!d.join("src").exists());
        assert_eq!(
            fs::read_to_string(d.join("dest/src/a.txt")).unwrap(),
            "aaaa"
        );
        assert_eq!(r.last.items_done, r.last.items_total);
    }

    #[test]
    fn cancelling_stops_and_leaves_no_part_file() {
        let d = scratch("cancel");
        fs::create_dir_all(d.join("dest")).unwrap();
        fs::write(d.join("big"), vec![7u8; 3 << 20]).unwrap();
        fs::write(d.join("small"), "s").unwrap();
        let ctl = std::sync::Arc::new(Control::default());
        let mut r = Auto::new(Choice::Skip);
        r.cancel_at = Some(1 << 20);
        r.ctl = Some(ctl.clone());
        let out = run(
            Kind::Copy,
            &[d.join("big"), d.join("small")],
            Some(&d.join("dest")),
            &ctl,
            &mut r,
        );
        assert!(out.cancelled);
        assert_eq!(
            fs::read_dir(d.join("dest")).unwrap().count(),
            0,
            "no partial file, nothing after"
        );
    }

    #[test]
    fn delete_removes_trees() {
        let d = scratch("delete");
        tree(&d);
        let mut r = Auto::new(Choice::Skip);
        let out = run(
            Kind::Delete,
            &[d.join("src")],
            None,
            &Control::default(),
            &mut r,
        );
        assert!(out.failed.is_empty());
        assert!(!d.join("src").exists());
        assert_eq!(r.last.items_done, 5);
    }

    #[test]
    fn rename_and_create() {
        let d = scratch("rename");
        fs::write(d.join("a.txt"), "").unwrap();
        fs::write(d.join("b.txt"), "").unwrap();
        assert!(
            rename(&d.join("a.txt"), "b.txt").is_err(),
            "won't overwrite"
        );
        assert_eq!(rename(&d.join("a.txt"), "c.txt").unwrap(), d.join("c.txt"));
        assert_eq!(
            create(&d, "New folder", false).unwrap(),
            d.join("New folder")
        );
        assert_eq!(
            create(&d, "New folder", false).unwrap(),
            d.join("New folder (2)")
        );
        assert!(create(&d, "c.txt", true).unwrap().ends_with("c (2).txt"));
    }
}
