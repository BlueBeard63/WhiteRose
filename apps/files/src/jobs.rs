//! `Ops` (QML singleton): file operations for QML. Copy, move, trash and
//! delete run as jobs on their own threads; copies and moves wait their turn
//! (one at a time is quicker on one disk), trash and delete go straight
//! away. `jobs` (JSON) is what the transfer panel shows; a job waiting on a
//! clash has `conflict` set until `answer` is called.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    unsafe extern "C++" {
        include!("whiterose-files/cpp/clip.h");
        #[cxx_name = "setClipboardFiles"]
        fn set_clipboard_files(uris: &str, cut: bool);
        #[cxx_name = "clipboardFiles"]
        fn clipboard_files() -> String;
        #[cxx_name = "keyboardModifiers"]
        fn keyboard_modifiers() -> i32;
        #[cxx_name = "setClipboardText"]
        fn set_clipboard_text(text: &str);
        #[cxx_name = "setLinkColour"]
        fn set_link_colour(colour: &str);
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        /// [{id, kind, label, state, bytesDone, bytesTotal, itemsDone,
        /// itemsTotal, current, rate, conflict, failed, cancelled}]
        /// state: waiting | running | paused | conflict | done
        #[qproperty(QString, jobs)]
        /// The accent (#rrggbb): links in drawn Markdown take it.
        #[qproperty(QString, accent)]
        type Ops = super::OpsRust;

        /// kind: copy | move | trash | delete; sources: JSON [path]; dest: a
        /// folder (copy, move). Returns the job's id.
        #[qinvokable]
        fn start(self: Pin<&mut Ops>, kind: &QString, sources: &QString, dest: &QString) -> i32;
        #[qinvokable]
        fn cancel(self: &Ops, id: i32);
        #[qinvokable]
        fn pause(self: &Ops, id: i32, on: bool);
        /// choice: replace | keep | skip; `all`: the same for later clashes.
        #[qinvokable]
        fn answer(self: &Ops, id: i32, choice: &QString, all: bool);
        /// Takes a finished job off the list.
        #[qinvokable]
        fn dismiss(self: Pin<&mut Ops>, id: i32);

        /// {ok, path} or {ok: false, error}.
        #[qinvokable]
        fn rename(self: &Ops, path: &QString, name: &QString) -> QString;
        /// A new folder (or empty file) in `dir`: {ok, path} or {ok: false, error}.
        #[qinvokable]
        fn create(self: &Ops, dir: &QString, name: &QString, file: bool) -> QString;

        /// Puts files on the clipboard (other file managers paste them too).
        #[qinvokable]
        #[cxx_name = "setClipboard"]
        fn set_clipboard(self: &Ops, paths: &QString, cut: bool);
        #[qinvokable]
        #[cxx_name = "setClipboardText"]
        fn copy_text(self: &Ops, text: &QString);
        /// {cut, paths} from the clipboard (paths empty if it holds no files).
        #[qinvokable]
        fn clipboard(self: &Ops) -> QString;
        /// The keyboard modifiers held now (Ctrl copies a drop, Shift moves it).
        #[qinvokable]
        fn modifiers(self: &Ops) -> i32;

        /// Puts things in the Trash back where they came from:
        /// {done: [path], failed: [[path, why]]}.
        #[qinvokable]
        fn restore(self: &Ops, paths: &QString) -> QString;
        /// Counts what's in these, folders all the way down, in the
        /// background; `measured` comes as it goes and at the end. Returns its id.
        #[qinvokable]
        fn measure(self: Pin<&mut Ops>, paths: &QString) -> i32;
        #[qinvokable]
        #[cxx_name = "stopMeasuring"]
        fn stop_measuring(self: &Ops, id: i32);
        /// {bytes, files, folders, done}
        #[qsignal]
        fn measured(self: Pin<&mut Ops>, id: i32, result: QString);

        /// Sizes these folders (all the way down), one after another in the
        /// background; `folderSized` for each. Asking again drops what's
        /// still waiting; sizes from the last minute come straight back.
        #[qinvokable]
        #[cxx_name = "sizeFolders"]
        fn size_folders(self: Pin<&mut Ops>, paths: &QString);
        #[qsignal]
        #[cxx_name = "folderSized"]
        fn folder_sized(self: Pin<&mut Ops>, path: QString, bytes: f64, items: f64);

        /// Git status of `dir` in the background; `gitStatus` brings
        /// {root, branch, ahead, behind, ignored, entries: {name: state}}.
        #[qinvokable]
        #[cxx_name = "askGit"]
        fn ask_git(self: &Ops, dir: &QString);
        #[qsignal]
        #[cxx_name = "gitStatus"]
        fn git_status(self: Pin<&mut Ops>, dir: QString, result: QString);

        /// Mounts a plugged-in drive in the background (the desktop may ask for
        /// your password); `driveDone` brings {device, path} or {device, error}.
        #[qinvokable]
        fn mount(self: &Ops, device: &QString);
        /// Unmounts and powers off a drive, likewise.
        #[qinvokable]
        fn eject(self: &Ops, device: &QString);
        #[qsignal]
        #[cxx_name = "driveDone"]
        fn drive_done(self: Pin<&mut Ops>, result: QString);

        /// MD5, SHA-1 and SHA-256 of a file in the background (one read).
        /// `hashed` brings {done: false, bytes, total} as it goes, then
        /// {done: true, md5, sha1, sha256} or {done: true, error}. Asking
        /// again (or "" ) stops the one before.
        #[qinvokable]
        fn hash(self: &Ops, path: &QString);
        #[qsignal]
        fn hashed(self: Pin<&mut Ops>, path: QString, result: QString);
        /// A Markdown file for showing: {ok, text, base, pending, hint}.
        /// Mermaid diagrams and LaTeX maths already drawn are images in
        /// `text`; with the tools installed the rest are drawn in the
        /// background and `rendered` says when to ask again; without them
        /// `hint` says what to install. `ink`: the text colour (#rrggbb).
        #[qinvokable]
        fn markdown(self: &Ops, path: &QString, dark: bool, ink: &QString) -> QString;
        #[qsignal]
        fn rendered(self: Pin<&mut Ops>, path: QString);
        /// Settings › Files › Verify copies.
        #[qinvokable]
        #[cxx_name = "setVerify"]
        fn set_verify(self: &Ops, on: bool);

        /// A job finished: {kind, done: [path], failed: [[path, why]], cancelled}.
        #[qsignal]
        fn finished(self: Pin<&mut Ops>, id: i32, result: QString);
    }

    impl cxx_qt::Threading for Ops {}
}

use std::{
    path::{Path, PathBuf},
    pin::Pin,
    sync::{
        atomic::Ordering,
        mpsc::{channel, Sender},
        Arc, Mutex,
    },
    time::{Duration, Instant},
};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use serde::Serialize;
use serde_json::{json, Value};

use crate::ops::{self, Choice, Control, Kind, Progress, Report};

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Conflict {
    src: String,
    dst: String,
    src_size: u64,
    dst_size: u64,
    src_modified: f64,
    dst_modified: f64,
    folders: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Job {
    id: i32,
    kind: &'static str,
    label: String,
    state: &'static str,
    #[serde(flatten)]
    progress: Progress,
    /// Bytes a second, since it started.
    rate: f64,
    conflict: Option<Conflict>,
    failed: Vec<(String, String)>,
    cancelled: bool,
    #[serde(skip)]
    control: Arc<Control>,
    #[serde(skip)]
    answers: Option<Sender<(Choice, bool)>>,
    #[serde(skip)]
    started: Option<Instant>,
}

#[derive(Default)]
pub struct OpsRust {
    jobs: QString,
    accent: QString,
    list: Arc<Mutex<Vec<Job>>>,
    /// Held by the copy or move that's running.
    transfers: Arc<Mutex<()>>,
    next: i32,
    /// Counts going on, each with its stop flag.
    measuring: Arc<Mutex<std::collections::HashMap<i32, Arc<std::sync::atomic::AtomicBool>>>>,
    sizer: Arc<Mutex<Sizer>>,
    /// Drawings being made (markdown.rs), so each is drawn once.
    drawing: Arc<Mutex<std::collections::HashSet<String>>>,
    /// The checksum being worked out, and its stop flag.
    hashing: Arc<Mutex<Option<Arc<std::sync::atomic::AtomicBool>>>>,
}

/// Folder sizes: what's waiting, what's known (and when), whether a thread
/// is working, and a number that changes when the waiting list does.
#[derive(Default)]
struct Sizer {
    waiting: std::collections::VecDeque<String>,
    known: std::collections::HashMap<String, (u64, u64, Instant)>,
    working: bool,
    round: u64,
}

/// Sizes kept this long.
const SIZE_FRESH: Duration = Duration::from_secs(60);

/// Bytes and items under `dir`, or None if `still` says stop.
fn folder_size(dir: &Path, still: &dyn Fn() -> bool) -> Option<(u64, u64)> {
    let (mut bytes, mut items) = (0u64, 0u64);
    let mut stack = vec![dir.to_path_buf()];
    let mut n = 0u32;
    while let Some(d) = stack.pop() {
        let Ok(rd) = std::fs::read_dir(&d) else {
            continue;
        };
        for e in rd.flatten() {
            n += 1;
            if n.is_multiple_of(512) && !still() {
                return None;
            }
            let Ok(m) = e.metadata() else { continue };
            items += 1;
            if m.is_dir() {
                stack.push(e.path());
            } else {
                bytes += m.len();
            }
        }
    }
    Some((bytes, items))
}

fn q(v: Value) -> QString {
    QString::from(v.to_string().as_str())
}

fn kind_name(k: Kind) -> &'static str {
    match k {
        Kind::Copy => "copy",
        Kind::Move => "move",
        Kind::Trash => "trash",
        Kind::Delete => "delete",
        Kind::Purge => "purge",
        Kind::Extract => "extract",
    }
}

fn shown_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string_lossy().into_owned())
}

/// "Copying notes.txt to Documents", "Moving 3 items to the Trash".
fn label(kind: Kind, sources: &[PathBuf], dest: Option<&Path>) -> String {
    let what = match sources {
        [one] => format!("“{}”", shown_name(one)),
        many => format!("{} items", many.len()),
    };
    let to = dest.map(shown_name).unwrap_or_default();
    match kind {
        Kind::Copy => format!("Copying {what} to {to}"),
        Kind::Move => format!("Moving {what} to {to}"),
        Kind::Trash => format!("Moving {what} to the Trash"),
        Kind::Delete => format!("Deleting {what}"),
        Kind::Purge => format!("Deleting {what} for good"),
        Kind::Extract => format!("Extracting {what}"),
    }
}

fn modified(m: &std::fs::Metadata) -> f64 {
    use std::os::unix::fs::MetadataExt;
    m.mtime() as f64 * 1000.0
}

/// The worker's side: updates its job and shows it (at most ~10 times a second).
struct Worker {
    id: i32,
    list: Arc<Mutex<Vec<Job>>>,
    thread: cxx_qt::CxxQtThread<qobject::Ops>,
    last: Instant,
}

impl Worker {
    fn update(&self, f: impl FnOnce(&mut Job)) {
        let mut l = self.list.lock().unwrap();
        if let Some(j) = l.iter_mut().find(|j| j.id == self.id) {
            f(j);
        }
    }
    fn publish(&mut self) {
        self.last = Instant::now();
        let _ = self.thread.queue(|mut obj| obj.as_mut().publish());
    }
}

impl Report for Worker {
    fn progress(&mut self, p: &Progress) {
        self.update(|j| {
            j.progress = p.clone();
            if let Some(t) = j.started {
                let secs = t.elapsed().as_secs_f64();
                if secs > 0.3 {
                    j.rate = p.bytes_done as f64 / secs;
                }
            }
            j.state = if j.control.pause.load(Ordering::Relaxed) {
                "paused"
            } else {
                "running"
            };
        });
        if self.last.elapsed() >= Duration::from_millis(100) {
            self.publish();
        }
    }

    fn conflict(&mut self, src: &Path, dst: &Path) -> (Choice, bool) {
        let (tx, rx) = channel();
        let a = std::fs::symlink_metadata(src).ok();
        let b = std::fs::symlink_metadata(dst).ok();
        let c = Conflict {
            src: src.to_string_lossy().into_owned(),
            dst: dst.to_string_lossy().into_owned(),
            src_size: a.as_ref().map(|m| m.len()).unwrap_or(0),
            dst_size: b.as_ref().map(|m| m.len()).unwrap_or(0),
            src_modified: a.as_ref().map(modified).unwrap_or(0.0),
            dst_modified: b.as_ref().map(modified).unwrap_or(0.0),
            folders: a.map(|m| m.is_dir()).unwrap_or(false)
                && b.map(|m| m.is_dir()).unwrap_or(false),
        };
        self.update(|j| {
            j.state = "conflict";
            j.conflict = Some(c);
            j.answers = Some(tx);
        });
        self.publish();
        let got = rx.recv().unwrap_or((Choice::Skip, true));
        self.update(|j| {
            j.state = "running";
            j.conflict = None;
            j.answers = None;
        });
        self.publish();
        got
    }
}

impl qobject::Ops {
    fn publish(self: Pin<&mut Self>) {
        let text = {
            let l = self.list.lock().unwrap();
            serde_json::to_string(&*l).unwrap_or_else(|_| "[]".into())
        };
        self.set_jobs(QString::from(text.as_str()));
    }

    pub fn start(
        mut self: Pin<&mut Self>,
        kind: &QString,
        sources: &QString,
        dest: &QString,
    ) -> i32 {
        let kind_text = kind.to_string();
        let Some(kind) = Kind::parse(&kind_text) else {
            return -1;
        };
        let sources: Vec<PathBuf> = serde_json::from_str::<Vec<String>>(&sources.to_string())
            .unwrap_or_default()
            .into_iter()
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .collect();
        if sources.is_empty() {
            return -1;
        }
        let dest = Some(PathBuf::from(dest.to_string())).filter(|d| d.is_absolute());
        self.as_mut().rust_mut().next += 1;
        let id = self.next;
        let control = Arc::new(Control::default());
        let queued = matches!(kind, Kind::Copy | Kind::Move);
        self.list.lock().unwrap().push(Job {
            id,
            kind: kind_name(kind),
            label: if kind_text == "empty" {
                "Emptying the Trash".into()
            } else {
                label(kind, &sources, dest.as_deref())
            },
            state: if queued { "waiting" } else { "running" },
            progress: Progress::default(),
            rate: 0.0,
            conflict: None,
            failed: vec![],
            cancelled: false,
            control: control.clone(),
            answers: None,
            started: None,
        });
        self.as_mut().publish();

        let list = self.list.clone();
        let transfers = self.transfers.clone();
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let _turn = queued.then(|| transfers.lock().unwrap_or_else(|e| e.into_inner()));
            let mut w = Worker {
                id,
                list,
                thread,
                last: Instant::now(),
            };
            w.update(|j| {
                j.started = Some(Instant::now());
                j.state = "running";
            });
            w.publish();
            let out = ops::run(kind, &sources, dest.as_deref(), &control, &mut w);
            // Folders' colours and icons go where they went (or go with them).
            if !out.moved.is_empty() || !out.gone.is_empty() {
                crate::looks::change(|l| {
                    let a = out.moved.iter().fold(false, |c, (f, t)| l.moved(f, t) | c);
                    out.gone.iter().fold(a, |c, g| l.forget(g) | c)
                });
                // And the list of what's tagged where.
                crate::tags::change(|r| {
                    let a = out.moved.iter().fold(false, |c, (f, t)| r.moved(f, t) | c);
                    out.gone.iter().fold(a, |c, g| r.forget(g) | c)
                });
            }
            w.update(|j| {
                j.state = "done";
                j.failed = out.failed.clone();
                j.cancelled = out.cancelled;
            });
            // In the log: what was done, and anything that went wrong.
            let what = format!(
                "{} {} item(s){}: {} done{}",
                kind_name(kind),
                sources.len(),
                dest.as_deref()
                    .map(|d| format!(" to {}", d.display()))
                    .unwrap_or_default(),
                out.done.len(),
                if out.cancelled { ", cancelled" } else { "" }
            );
            if out.failed.is_empty() {
                crate::log::info(&what);
            } else {
                crate::log::warn(&what);
                for (p, why) in out.failed.iter().take(20) {
                    crate::log::warn(&format!("  {p}: {why}"));
                }
            }
            let result = json!({
                "kind": kind_name(kind),
                "done": out.done,
                "failed": out.failed,
                "cancelled": out.cancelled,
            })
            .to_string();
            let _ = w.thread.queue(move |mut obj| {
                obj.as_mut().publish();
                obj.as_mut().finished(id, QString::from(result.as_str()));
            });
        });
        id
    }

    fn with_job(&self, id: i32, f: impl FnOnce(&mut Job)) {
        if let Some(j) = self.list.lock().unwrap().iter_mut().find(|j| j.id == id) {
            f(j);
        }
    }

    pub fn cancel(&self, id: i32) {
        self.with_job(id, |j| {
            j.control.cancel.store(true, Ordering::Relaxed);
            // Waiting on a clash: that's an answer too.
            if let Some(tx) = j.answers.take() {
                let _ = tx.send((Choice::Skip, true));
            }
        });
    }

    pub fn pause(&self, id: i32, on: bool) {
        self.with_job(id, |j| j.control.pause.store(on, Ordering::Relaxed));
    }

    pub fn answer(&self, id: i32, choice: &QString, all: bool) {
        let choice = Choice::parse(&choice.to_string());
        self.with_job(id, |j| {
            if let Some(tx) = j.answers.take() {
                let _ = tx.send((choice, all));
            }
        });
    }

    pub fn dismiss(mut self: Pin<&mut Self>, id: i32) {
        self.list
            .lock()
            .unwrap()
            .retain(|j| j.id != id || j.state != "done");
        self.as_mut().publish();
    }

    pub fn rename(&self, path: &QString, name: &QString) -> QString {
        let from = path.to_string();
        match ops::rename(Path::new(&from), &name.to_string()) {
            Ok(p) => {
                let to = p.to_string_lossy().into_owned();
                crate::looks::change(|l| l.moved(&from, &to));
                crate::tags::change(|r| r.moved(&from, &to));
                q(json!({"ok": true, "path": p}))
            }
            Err(e) => q(json!({"ok": false, "error": e})),
        }
    }

    pub fn create(&self, dir: &QString, name: &QString, file: bool) -> QString {
        match ops::create(Path::new(&dir.to_string()), &name.to_string(), file) {
            Ok(p) => q(json!({"ok": true, "path": p})),
            Err(e) => q(json!({"ok": false, "error": e})),
        }
    }

    pub fn restore(&self, paths: &QString) -> QString {
        let paths: Vec<String> = serde_json::from_str(&paths.to_string()).unwrap_or_default();
        let (mut done, mut failed) = (vec![], vec![]);
        for p in paths {
            match crate::trash::restore(Path::new(&p)) {
                Ok(to) => {
                    let to = to.to_string_lossy().into_owned();
                    crate::looks::change(|l| l.moved(&p, &to));
                    crate::tags::change(|r| r.moved(&p, &to));
                    done.push(to);
                }
                Err(e) => failed.push((p, e)),
            }
        }
        q(json!({"done": done, "failed": failed}))
    }

    pub fn measure(mut self: Pin<&mut Self>, paths: &QString) -> i32 {
        let paths: Vec<PathBuf> = serde_json::from_str::<Vec<String>>(&paths.to_string())
            .unwrap_or_default()
            .into_iter()
            .map(PathBuf::from)
            .collect();
        self.as_mut().rust_mut().next += 1;
        let id = self.next;
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.measuring.lock().unwrap().insert(id, stop.clone());
        let measuring = self.measuring.clone();
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let (mut bytes, mut files, mut folders) = (0u64, 0u64, 0u64);
            let mut last = Instant::now();
            let send = |bytes: u64, files: u64, folders: u64, done: bool| {
                let r = json!({"bytes": bytes, "files": files, "folders": folders, "done": done})
                    .to_string();
                let _ = thread
                    .queue(move |mut obj| obj.as_mut().measured(id, QString::from(r.as_str())));
            };
            // What's inside the folders asked about (not the folders themselves).
            let mut stack: Vec<PathBuf> = vec![];
            for p in &paths {
                match std::fs::symlink_metadata(p) {
                    Ok(m) if m.is_dir() => stack.push(p.clone()),
                    Ok(m) => {
                        bytes += m.len();
                        files += 1;
                    }
                    Err(_) => {}
                }
            }
            while let Some(dir) = stack.pop() {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                let Ok(rd) = std::fs::read_dir(&dir) else {
                    continue;
                };
                for e in rd.flatten() {
                    let Ok(m) = e.metadata() else { continue };
                    if m.is_dir() {
                        folders += 1;
                        stack.push(e.path());
                    } else {
                        bytes += m.len();
                        files += 1;
                    }
                }
                if last.elapsed() >= Duration::from_millis(150) {
                    last = Instant::now();
                    send(bytes, files, folders, false);
                }
            }
            send(bytes, files, folders, true);
            measuring.lock().unwrap().remove(&id);
        });
        id
    }

    pub fn size_folders(self: Pin<&mut Self>, paths: &QString) {
        let paths: Vec<String> = serde_json::from_str(&paths.to_string()).unwrap_or_default();
        let mut fresh = vec![];
        let start = {
            let mut s = self.sizer.lock().unwrap();
            s.round += 1;
            s.waiting.clear();
            for p in paths {
                match s.known.get(&p) {
                    Some(&(b, i, at)) if at.elapsed() < SIZE_FRESH => fresh.push((p, b, i)),
                    _ => s.waiting.push_back(p),
                }
            }
            let start = !s.working && !s.waiting.is_empty();
            s.working |= start;
            start
        };
        let mut this = self;
        for (p, b, i) in fresh {
            this.as_mut()
                .folder_sized(QString::from(p.as_str()), b as f64, i as f64);
        }
        if !start {
            return;
        }
        let sizer = this.sizer.clone();
        let thread = this.qt_thread();
        std::thread::spawn(move || {
            crate::finder::background();
            loop {
                let (next, round) = {
                    let mut s = sizer.lock().unwrap();
                    match s.waiting.pop_front() {
                        Some(p) => (p, s.round),
                        None => {
                            s.working = false;
                            return;
                        }
                    }
                };
                let still = || sizer.lock().map(|s| s.round == round).unwrap_or(false);
                let Some((b, i)) = folder_size(Path::new(&next), &still) else {
                    continue;
                };
                sizer
                    .lock()
                    .unwrap()
                    .known
                    .insert(next.clone(), (b, i, Instant::now()));
                let _ = thread.queue(move |mut obj| {
                    obj.as_mut()
                        .folder_sized(QString::from(next.as_str()), b as f64, i as f64)
                });
            }
        });
    }

    pub fn mount(&self, device: &QString) {
        let device = device.to_string();
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let r = match crate::places::mount(&device) {
                Ok(p) => {
                    crate::log::info(&format!("mounted {device} at {p}"));
                    json!({"device": device, "action": "mount", "path": p})
                }
                Err(e) => {
                    crate::log::warn(&format!("couldn't mount {device}: {e}"));
                    json!({"device": device, "action": "mount", "error": e})
                }
            }
            .to_string();
            let _ = thread.queue(move |mut obj| obj.as_mut().drive_done(QString::from(r.as_str())));
        });
    }

    pub fn eject(&self, device: &QString) {
        let device = device.to_string();
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let r = match crate::places::eject(&device) {
                Ok(()) => {
                    crate::log::info(&format!("ejected {device}"));
                    json!({"device": device, "action": "eject"})
                }
                Err(e) => {
                    crate::log::warn(&format!("couldn't eject {device}: {e}"));
                    json!({"device": device, "action": "eject", "error": e})
                }
            }
            .to_string();
            let _ = thread.queue(move |mut obj| obj.as_mut().drive_done(QString::from(r.as_str())));
        });
    }

    pub fn ask_git(&self, dir: &QString) {
        let dir = dir.to_string();
        if !dir.starts_with('/') {
            return;
        }
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            crate::finder::background();
            let r = serde_json::to_string(&crate::git::status(Path::new(&dir))).unwrap_or_default();
            let _ = thread.queue(move |mut obj| {
                obj.as_mut()
                    .git_status(QString::from(dir.as_str()), QString::from(r.as_str()))
            });
        });
    }

    pub fn stop_measuring(&self, id: i32) {
        if let Some(f) = self.measuring.lock().unwrap().get(&id) {
            f.store(true, Ordering::Relaxed);
        }
    }

    pub fn copy_text(&self, text: &QString) {
        qobject::set_clipboard_text(&text.to_string());
    }

    pub fn set_clipboard(&self, paths: &QString, cut: bool) {
        let paths: Vec<String> = serde_json::from_str(&paths.to_string()).unwrap_or_default();
        let uris: Vec<String> = paths
            .iter()
            .map(|p| format!("file://{}", crate::places::percent_encode(p)))
            .collect();
        qobject::set_clipboard_files(&uris.join("\n"), cut);
    }

    pub fn modifiers(&self) -> i32 {
        qobject::keyboard_modifiers()
    }

    pub fn clipboard(&self) -> QString {
        let text = qobject::clipboard_files();
        let mut lines = text.lines();
        let cut = lines.next() == Some("cut");
        let paths: Vec<String> = lines
            .filter_map(|l| l.strip_prefix("file://"))
            .map(crate::places::percent_decode)
            .filter(|p| p.starts_with('/'))
            .collect();
        q(json!({"cut": cut, "paths": paths}))
    }
}

impl qobject::Ops {
    pub fn markdown(&self, path: &QString, dark: bool, ink: &QString) -> QString {
        use crate::markdown as md;
        let p = path.to_string();
        let ink = ink.to_string();
        // (Links in drawn Markdown take the accent.)
        qobject::set_link_colour(&self.accent.to_string());
        let text = match std::fs::metadata(&p) {
            Ok(m) if m.len() > 4 << 20 => {
                return q(json!({"ok": false, "error": "Too big to show (over 4 MB)"}))
            }
            Ok(_) => match std::fs::read_to_string(&p) {
                Ok(t) => t,
                Err(e) => return q(json!({"ok": false, "error": e.to_string()})),
            },
            Err(e) => return q(json!({"ok": false, "error": e.to_string()})),
        };
        let cache = md::cache_dir();
        let (out, missing) = md::with_drawings(&text, &cache, dark, &ink);
        let (mermaid, maths) = md::tools(&|n| crate::launch::installed(n));
        let drawable: Vec<md::Piece> = missing
            .iter()
            .filter(|x| {
                if x.kind == md::Kind::Mermaid {
                    mermaid
                } else {
                    maths
                }
            })
            .cloned()
            .collect();
        let mut hint = vec![];
        if !mermaid && missing.iter().any(|x| x.kind == md::Kind::Mermaid) {
            hint.push("diagrams: install mermaid-cli (mmdc)");
        }
        if !maths && missing.iter().any(|x| x.kind != md::Kind::Mermaid) {
            hint.push("maths: install TeX Live (latex, dvisvgm)");
        }
        let base = Path::new(&p)
            .parent()
            .map(|d| format!("file://{}/", d.display()))
            .unwrap_or_default();
        // Draw what can be, once each, then tell QML to ask again.
        let todo: Vec<md::Piece> = {
            let mut busy = self.drawing.lock().unwrap();
            drawable
                .iter()
                .filter(|x| busy.insert(md::key(x, dark, &ink)))
                .cloned()
                .collect()
        };
        if !todo.is_empty() {
            let thread = self.qt_thread();
            let busy = self.drawing.clone();
            let path = p.clone();
            std::thread::spawn(move || {
                for x in &todo {
                    if let Err(e) = md::draw(x, &cache, dark, &ink) {
                        crate::log::warn(&format!("drawing a {:?} in {path}: {e}", x.kind));
                    }
                    busy.lock().unwrap().remove(&md::key(x, dark, &ink));
                }
                let _ = thread
                    .queue(move |mut obj| obj.as_mut().rendered(QString::from(path.as_str())));
            });
        }
        q(
            json!({"ok": true, "text": out, "base": base, "pending": !drawable.is_empty(), "hint": if hint.is_empty() { String::new() } else { format!("To see {}", hint.join("; ")) }}),
        )
    }

    pub fn set_verify(&self, on: bool) {
        crate::ops::VERIFY.store(on, Ordering::Relaxed);
    }

    pub fn hash(&self, path: &QString) {
        use std::sync::atomic::AtomicBool;
        let stop = Arc::new(AtomicBool::new(false));
        if let Some(old) = self.hashing.lock().unwrap().replace(stop.clone()) {
            old.store(true, Ordering::Relaxed);
        }
        let path = path.to_string();
        if path.is_empty() {
            return;
        }
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let p = PathBuf::from(&path);
            let total = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            let tell = |r: serde_json::Value| {
                let (path, r) = (path.clone(), r.to_string());
                let _ = thread.queue(move |mut obj| {
                    obj.as_mut()
                        .hashed(QString::from(path.as_str()), QString::from(r.as_str()))
                });
            };
            let mut last = Instant::now();
            let r = crate::hash::sums(&p, &stop, &mut |done| {
                if last.elapsed() > Duration::from_millis(150) {
                    last = Instant::now();
                    tell(json!({"done": false, "bytes": done, "total": total}));
                }
            });
            match r {
                Ok(Some(s)) => {
                    tell(json!({"done": true, "md5": s.md5, "sha1": s.sha1, "sha256": s.sha256}))
                }
                Ok(None) => {}
                Err(e) => tell(json!({"done": true, "error": e.to_string()})),
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels() {
        let a = [PathBuf::from("/x/notes.txt")];
        let many = [PathBuf::from("/x/a"), PathBuf::from("/x/b")];
        assert_eq!(
            label(Kind::Copy, &a, Some(Path::new("/home/me/Documents"))),
            "Copying “notes.txt” to Documents"
        );
        assert_eq!(
            label(Kind::Move, &many, Some(Path::new("/tmp"))),
            "Moving 2 items to tmp"
        );
        assert_eq!(
            label(Kind::Trash, &many, None),
            "Moving 2 items to the Trash"
        );
        assert_eq!(label(Kind::Delete, &a, None), "Deleting “notes.txt”");
    }

    #[test]
    fn folder_sizes() {
        let d = std::env::temp_dir().join(format!("wr-sizes-{}", std::process::id()));
        std::fs::create_dir_all(d.join("a/b")).unwrap();
        std::fs::write(d.join("a/one"), "12345").unwrap();
        std::fs::write(d.join("a/b/two"), "123").unwrap();
        assert_eq!(
            folder_size(&d.join("a"), &|| true),
            Some((8, 3)),
            "two files and a folder"
        );
        assert_eq!(folder_size(&d.join("missing"), &|| true), Some((0, 0)));
        // Told to stop: no answer (checked every 512 entries).
        for i in 0..600 {
            std::fs::write(d.join(format!("a/f{i}")), "").unwrap();
        }
        assert_eq!(folder_size(&d.join("a"), &|| false), None);
    }
}
