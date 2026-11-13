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
        /// Makes the archive `dest` (zip, 7z, tar.gz, tar.xz, tar.zst, tar.bz2:
        /// from its name) of `sources` (JSON [path]); a password for zip and
        /// 7z ("" none), split into parts of `split` bytes (0: whole). A job.
        #[qinvokable]
        fn compress(
            self: Pin<&mut Ops>,
            sources: &QString,
            dest: &QString,
            password: &QString,
            split: f64,
        ) -> i32;
        /// The password for an archive (kept in memory this run; extracting
        /// says archive::NEEDS_PASSWORD when it's wanted).
        #[qinvokable]
        #[cxx_name = "setArchivePassword"]
        fn set_archive_password(self: &Ops, archive: &QString, password: &QString);
        /// Takes a finished job off the list.
        #[qinvokable]
        fn dismiss(self: Pin<&mut Ops>, id: i32);

        /// {ok, path} or {ok: false, error}.
        #[qinvokable]
        fn rename(self: &Ops, path: &QString, name: &QString) -> QString;
        /// What changes, line by line, if `there` is replaced by `coming` (text
        /// files up to 1 MB): {ok, hunks: [[{kind, text, old, new}]], added,
        /// removed} or {ok: false, error}.
        #[qinvokable]
        fn diff(self: &Ops, there: &QString, coming: &QString) -> QString;
        /// Rename… with several: every new name under the rules
        /// (bulk_rename::Rules, JSON), in the order given, with problems:
        /// {ok, items: [{from, to, problem}]} or {ok: false, error}.
        /// `dirs`: which paths are folders (JSON bools).
        #[qinvokable]
        #[cxx_name = "renamePreview"]
        fn rename_preview(self: &Ops, paths: &QString, dirs: &QString, rules: &QString) -> QString;
        /// Renames them (worked out again now, never from an old preview):
        /// {done: [[from, to]], failed: [[from, error]]}.
        #[qinvokable]
        #[cxx_name = "renameMany"]
        fn rename_many(self: &Ops, paths: &QString, dirs: &QString, rules: &QString) -> QString;
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

        /// Puts things in the trash back where they came from:
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

        /// Clean up's scans (cleanup.rs) of `root` in the background: kind
        /// largest | duplicates | caches. `cleanupFound` brings {done: false,
        /// scanned} as it goes, then {done: true, files} (largest, caches:
        /// [{path, size, modified, what}]) or {done: true, groups: [{size,
        /// files}]} (duplicates). Returns its id; `stopCleanup` stops it.
        #[qinvokable]
        fn cleanup(self: Pin<&mut Ops>, kind: &QString, root: &QString) -> i32;
        #[qinvokable]
        #[cxx_name = "stopCleanup"]
        fn stop_cleanup(self: &Ops, id: i32);
        #[qsignal]
        #[cxx_name = "cleanupFound"]
        fn cleanup_found(self: Pin<&mut Ops>, id: i32, result: QString);

        /// Connects to a network share (shares.rs) in the background:
        /// `request` {address, user, domain, password, remember, save,
        /// accept}, or {account: id, address} for an online account's drive. `connected` brings {result: ok, path, uri, name} |
        /// password {wrong} | question {text} | failed {error}.
        #[qinvokable]
        #[cxx_name = "connectServer"]
        fn connect(self: Pin<&mut Ops>, request: &QString) -> i32;
        #[qsignal]
        #[cxx_name = "serverConnected"]
        fn connected(self: Pin<&mut Ops>, id: i32, result: QString);
        /// (Cancel a connection with stopCleanup.)
        /// Unmounts a share: "" or why not.
        #[qinvokable]
        #[cxx_name = "disconnectServer"]
        fn disconnect(self: &Ops, uri: &QString) -> QString;
        /// Saved servers, online accounts' drives (GNOME Online Accounts:
        /// {account: id}) and shares mounted now: [{name, uri, user,
        /// remembered, saved, account, path}] (path: "" if not mounted).
        #[qinvokable]
        fn servers(self: &Ops) -> QString;
        /// The same, in the background (it runs gio, which waits on the
        /// drives); `serversListed` brings it.
        #[qinvokable]
        #[cxx_name = "askServers"]
        fn ask_servers(self: &Ops);
        #[qsignal]
        #[cxx_name = "serversListed"]
        fn servers_listed(self: Pin<&mut Ops>, result: QString);
        /// Cloud accounts through rclone (cloudremotes.rs): {rclone: is it
        /// installed, providers: [{kind, name}]}.
        #[qinvokable]
        #[cxx_name = "cloudProviders"]
        fn cloud_providers(self: &Ops) -> QString;
        /// One step of adding a cloud account, in the background: `request`
        /// {name, kind, params: {key: value}} to start, or {name, state,
        /// answer} to carry on. `cloudSetupStep` brings {note} (a sign-in
        /// link) along the way, then {step: done | ask {...} | failed {error}}.
        /// Stopped with stopCleanup.
        #[qinvokable]
        #[cxx_name = "cloudSetup"]
        fn cloud_setup(self: Pin<&mut Ops>, request: &QString) -> i32;
        #[qsignal]
        #[cxx_name = "cloudSetupStep"]
        fn cloud_setup_step(self: Pin<&mut Ops>, id: i32, result: QString);
        /// Docker or Podman containers (containers.rs), in the background:
        /// `containersFound` brings {engine, containers: [...]} or {engine,
        /// error} ("" engine: neither is installed).
        #[qinvokable]
        #[cxx_name = "askContainers"]
        fn ask_containers(self: &Ops);
        #[qsignal]
        #[cxx_name = "containersFound"]
        fn containers_found(self: Pin<&mut Ops>, result: QString);
        /// Something done to a container, in the background: `request`
        /// {what: start|stop|restart|pause|unpause|remove, id} or
        /// {what: compose, action: up|down|restart|pull, workdir} or
        /// {what: copy, id, path, dest}. `containerDone` brings {ok, error?, path?}.
        #[qinvokable]
        #[cxx_name = "containerDo"]
        fn container_do(self: Pin<&mut Ops>, request: &QString) -> i32;
        #[qsignal]
        #[cxx_name = "containerDone"]
        fn container_done(self: Pin<&mut Ops>, id: i32, result: QString);
        /// What's in `path` inside a running container: {entries} or {error}.
        #[qinvokable]
        #[cxx_name = "containerLs"]
        fn container_ls(self: &Ops, id: &QString, path: &QString) -> QString;
        /// The terminal command for a container's logs or a shell ("" if not).
        #[qinvokable]
        #[cxx_name = "containerCommand"]
        fn container_command(self: &Ops, what: &QString, id: &QString) -> QString;
        /// Settings › Files › Encrypt search index and history (secure.rs):
        /// switched, and what's already there rewritten the new way (in the
        /// background).
        #[qinvokable]
        #[cxx_name = "setEncrypt"]
        fn set_encrypt(self: &Ops, on: bool);
        /// Copies and moves that didn't finish last time (journal.rs):
        /// [{key, kind, dest, started, remaining: [path]}].
        #[qinvokable]
        fn interrupted(self: &Ops) -> QString;
        /// Carries on with one (half-written files cleared): its job id, -1
        /// if nothing's left to do.
        #[qinvokable]
        #[cxx_name = "resumeTransfer"]
        fn resume_transfer(self: Pin<&mut Ops>, key: &QString) -> i32;
        #[qinvokable]
        #[cxx_name = "forgetTransfer"]
        fn forget_transfer(self: &Ops, key: &QString);
        /// Saves Files' settings to `path` (prefs.rs): "" or why not.
        #[qinvokable]
        #[cxx_name = "exportSettings"]
        fn export_settings(self: &Ops, path: &QString) -> QString;
        /// Brings settings in from `path` (what was there is kept in
        /// before-import.json): {ok, settings, files} or {ok: false, error}.
        #[qinvokable]
        #[cxx_name = "importSettings"]
        fn import_settings(self: &Ops, path: &QString) -> QString;
        /// Hosts named in ~/.ssh/config (JSON [name]).
        #[qinvokable]
        #[cxx_name = "sshHosts"]
        fn ssh_hosts(self: &Ops) -> QString;
        /// Takes a server off the list (and its password out of the keyring).
        #[qinvokable]
        #[cxx_name = "forgetServer"]
        fn forget_server(self: &Ops, uri: &QString);

        /// Compares two folders in the background (compare.rs), byte by
        /// byte when `content`. `compared` brings {done: false, scanned} as
        /// it goes, then {done: true, entries}. Returns its id; stopped with
        /// `stopCleanup`.
        #[qinvokable]
        fn compare(self: Pin<&mut Ops>, left: &QString, right: &QString, content: bool) -> i32;
        #[qsignal]
        fn compared(self: Pin<&mut Ops>, id: i32, result: QString);
        /// What a sync would do with compared `entries` (JSON): mode update |
        /// mirror | two-way, towards right | left. A compare::Plan.
        #[qinvokable]
        #[cxx_name = "syncPlan"]
        fn sync_plan(
            self: &Ops,
            left: &QString,
            right: &QString,
            entries: &QString,
            mode: &QString,
            towards: &QString,
        ) -> QString;
        /// Carries out a plan from `syncPlan` as a job. Returns its id.
        #[qinvokable]
        fn sync(self: Pin<&mut Ops>, plan: &QString) -> i32;

        /// Sizes these folders (all the way down), one after another in the
        /// background; `folderSized` for each. Asking again drops what's
        /// still waiting; sizes from the last minute come straight back.
        #[qinvokable]
        #[cxx_name = "sizeFolders"]
        fn size_folders(self: Pin<&mut Ops>, paths: &QString);
        #[qsignal]
        #[cxx_name = "folderSized"]
        fn folder_sized(self: Pin<&mut Ops>, path: QString, bytes: f64, items: f64);

        /// Sync-client state of what's in `dir` (cloudsync.rs) in the
        /// background; `cloudStatus` brings {client, root, entries: {name:
        /// state}}, client "" when no sync client keeps it.
        #[qinvokable]
        #[cxx_name = "askCloud"]
        fn ask_cloud(self: &Ops, dir: &QString);
        #[qsignal]
        #[cxx_name = "cloudStatus"]
        fn cloud_status(self: Pin<&mut Ops>, dir: QString, result: QString);
        /// Asks the sync client to keep `paths` (JSON) on this device
        /// ("local"), free up their space ("online"), share or copy a link
        /// to the first ("share", "link"): "" or why not.
        #[qinvokable]
        #[cxx_name = "cloudAct"]
        fn cloud_act(self: &Ops, what: &QString, paths: &QString) -> QString;

        /// A folder on a network drive listed in the background (Fs.list's
        /// answer, without the window waiting on the server); `folderListed`
        /// brings it with the request (to drop answers for folders left).
        #[qinvokable]
        #[cxx_name = "listFolder"]
        fn list_folder(
            self: &Ops,
            path: &QString,
            hidden: bool,
            sort: &QString,
            descending: bool,
            icon_size: i32,
            folders: &QString,
        );
        #[qsignal]
        #[cxx_name = "folderListed"]
        fn folder_listed(self: Pin<&mut Ops>, path: QString, result: QString);
        /// Fs.stamp in the background (a network drive's change check).
        #[qinvokable]
        #[cxx_name = "folderStamp"]
        fn folder_stamp(self: &Ops, path: &QString);
        #[qsignal]
        #[cxx_name = "folderStamped"]
        fn folder_stamped(self: Pin<&mut Ops>, path: QString, stamp: f64);

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
        /// Settings › Files › Check copies.
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
    /// Both small text files: their lines can be compared (Ops.diff).
    text: bool,
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
        Kind::Compress => "compress",
        Kind::Sync => "sync",
    }
}

fn shown_name(p: &Path) -> String {
    p.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| p.to_string_lossy().into_owned())
}

/// "Copying notes.txt to Documents", "Moving 3 items to the trash".
fn label(kind: Kind, sources: &[PathBuf], dest: Option<&Path>) -> String {
    let what = match sources {
        [one] => format!("“{}”", shown_name(one)),
        many => format!("{} items", many.len()),
    };
    let to = dest.map(shown_name).unwrap_or_default();
    match kind {
        // (Copying out of an archive takes things out of it.)
        Kind::Copy
            if sources
                .iter()
                .all(|s| s.to_string_lossy().starts_with(crate::archive::PREFIX)) =>
        {
            format!("Extracting {what} to {to}")
        }
        Kind::Copy => format!("Copying {what} to {to}"),
        Kind::Move => format!("Moving {what} to {to}"),
        Kind::Trash => format!("Moving {what} to the trash"),
        Kind::Delete => format!("Deleting {what}"),
        Kind::Purge => format!("Deleting {what} for good"),
        Kind::Extract => format!("Extracting {what}"),
        Kind::Compress => format!("Compressing {what} into {to}"),
        Kind::Sync => match sources {
            [l, r] => format!("Syncing “{}” and “{}”", shown_name(l), shown_name(r)),
            _ => "Syncing".into(),
        },
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
            folders: a.as_ref().map(|m| m.is_dir()).unwrap_or(false)
                && b.as_ref().map(|m| m.is_dir()).unwrap_or(false),
            text: [(src, &a), (dst, &b)].iter().all(|(p, m)| {
                m.as_ref()
                    .is_some_and(|m| m.is_file() && m.len() <= crate::diff::MAX)
                    && std::fs::File::open(p)
                        .and_then(|mut f| {
                            let mut head = vec![0u8; 8192];
                            let n = std::io::Read::read(&mut f, &mut head)?;
                            Ok(crate::diff::is_text(&head[..n]))
                        })
                        .unwrap_or(false)
            }),
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

    pub fn start(self: Pin<&mut Self>, kind: &QString, sources: &QString, dest: &QString) -> i32 {
        let kind_text = kind.to_string();
        let Some(kind) = Kind::parse(&kind_text) else {
            return -1;
        };
        self.launch(kind, &kind_text, sources, dest, Control::default())
    }

    pub fn compress(
        self: Pin<&mut Self>,
        sources: &QString,
        dest: &QString,
        password: &QString,
        split: f64,
    ) -> i32 {
        let pw = password.to_string();
        let control = Control::default();
        *control.archive.lock().unwrap() = ((!pw.is_empty()).then_some(pw), split.max(0.0) as u64);
        self.launch(Kind::Compress, "compress", sources, dest, control)
    }

    pub fn set_archive_password(&self, archive: &QString, password: &QString) {
        crate::archive::set_password(Path::new(&archive.to_string()), &password.to_string());
    }

    fn launch(
        mut self: Pin<&mut Self>,
        kind: Kind,
        kind_text: &str,
        sources: &QString,
        dest: &QString,
        control: Control,
    ) -> i32 {
        let sources: Vec<PathBuf> = serde_json::from_str::<Vec<String>>(&sources.to_string())
            .unwrap_or_default()
            .into_iter()
            .map(PathBuf::from)
            // (Things inside an archive are taken out when copied.)
            .filter(|p| p.is_absolute() || p.to_string_lossy().starts_with(crate::archive::PREFIX))
            .collect();
        if sources.is_empty() {
            return -1;
        }
        let dest = Some(PathBuf::from(dest.to_string())).filter(|d| d.is_absolute());
        self.as_mut().rust_mut().next += 1;
        let id = self.next;
        let control = Arc::new(control);
        let queued = matches!(kind, Kind::Copy | Kind::Move | Kind::Sync);
        self.list.lock().unwrap().push(Job {
            id,
            kind: kind_name(kind),
            label: if kind_text == "empty" {
                "Emptying the trash".into()
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

        // Copies and moves are written down until they end (journal.rs).
        let journal_key = (matches!(kind, Kind::Copy | Kind::Move)
            && sources.iter().all(|p| p.is_absolute())
            && dest.is_some())
        .then(|| {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let key = format!("{now}-{}-{id}", std::process::id());
            crate::journal::start(
                &crate::journal::file(),
                crate::journal::Entry {
                    key: key.clone(),
                    kind: kind_name(kind).into(),
                    sources: sources
                        .iter()
                        .map(|p| p.to_string_lossy().into_owned())
                        .collect(),
                    dest: dest
                        .as_ref()
                        .map(|d| d.to_string_lossy().into_owned())
                        .unwrap_or_default(),
                    started: now,
                },
            );
            key
        });
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
            if let Some(k) = &journal_key {
                crate::journal::finish(&crate::journal::file(), k);
            }
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

    pub fn diff(&self, there: &QString, coming: &QString) -> QString {
        match crate::diff::files(
            Path::new(&there.to_string()),
            Path::new(&coming.to_string()),
        ) {
            Ok(d) => {
                q(json!({"ok": true, "hunks": d.hunks, "added": d.added, "removed": d.removed}))
            }
            Err(e) => q(json!({"ok": false, "error": e})),
        }
    }

    pub fn rename_preview(&self, paths: &QString, dirs: &QString, rules: &QString) -> QString {
        match bulk_plan(paths, dirs, rules) {
            Ok(items) => q(json!({"ok": true, "items": items})),
            Err(e) => q(json!({"ok": false, "error": e})),
        }
    }

    pub fn rename_many(&self, paths: &QString, dirs: &QString, rules: &QString) -> QString {
        let plan = match bulk_plan(paths, dirs, rules) {
            Ok(p) => p,
            Err(e) => return q(json!({"done": [], "failed": [["", e]]})),
        };
        let (done, failed) = crate::bulk_rename::apply(&plan);
        // Colours, icons and tags follow the files.
        for (from, to) in &done {
            crate::looks::change(|l| l.moved(from, to));
            crate::tags::change(|r| r.moved(from, to));
        }
        q(json!({"done": done, "failed": failed}))
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

    pub fn ask_cloud(&self, dir: &QString) {
        let dir = dir.to_string();
        if !dir.starts_with('/') {
            return;
        }
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            crate::finder::background();
            let run = std::env::var_os("XDG_RUNTIME_DIR")
                .map(PathBuf::from)
                .unwrap_or_default();
            let st = crate::cloudsync::status(&run, Path::new(&dir), 2000).unwrap_or_default();
            let r = serde_json::to_string(&st).unwrap_or_default();
            let _ = thread.queue(move |mut obj| {
                obj.as_mut()
                    .cloud_status(QString::from(dir.as_str()), QString::from(r.as_str()))
            });
        });
    }

    pub fn cloud_act(&self, what: &QString, paths: &QString) -> QString {
        let paths: Vec<PathBuf> = serde_json::from_str::<Vec<String>>(&paths.to_string())
            .unwrap_or_default()
            .into_iter()
            .map(PathBuf::from)
            .collect();
        let run = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_default();
        match crate::cloudsync::act(&run, &what.to_string(), &paths) {
            Ok(()) => QString::from(""),
            Err(e) => QString::from(e.as_str()),
        }
    }

    pub fn list_folder(
        &self,
        path: &QString,
        hidden: bool,
        sort: &QString,
        descending: bool,
        icon_size: i32,
        folders: &QString,
    ) {
        let (path, sort, folders) = (path.to_string(), sort.to_string(), folders.to_string());
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let r = crate::core::list_value(&path, hidden, &sort, descending, icon_size, &folders)
                .to_string();
            let _ = thread.queue(move |mut obj| {
                obj.as_mut()
                    .folder_listed(QString::from(path.as_str()), QString::from(r.as_str()))
            });
        });
    }

    pub fn folder_stamp(&self, path: &QString) {
        let path = path.to_string();
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let s = crate::core::stamp_of(&path);
            let _ = thread
                .queue(move |mut obj| obj.as_mut().folder_stamped(QString::from(path.as_str()), s));
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

/// The sidebar's network list (Ops.servers / askServers). It runs gio
/// (`gio mount -l`, `gio info` per mounted drive: half a second or more),
/// so the sidebar asks for it on a thread.
pub fn servers_json() -> String {
    use crate::shares;
    let saved = shares::load(&shares::servers_file());
    let now = shares::mounted("gio");
    // A mount's address may carry the user (sftp://jack@build).
    let bare = |u: &str| match shares::parse(u) {
        Ok(a) => shares::Address {
            user: String::new(),
            ..a
        }
        .uri(),
        Err(_) => u.to_string(),
    };
    let path_of = |uri: &str| -> String {
        match now.iter().find(|(u, _)| bare(u) == uri) {
            Some((u, _)) => shares::local_path("gio", u)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default(),
            None => String::new(),
        }
    };
    let mut out: Vec<Value> = saved
        .iter()
        .map(|s| json!({"name": s.name, "uri": s.uri, "user": s.user, "domain": s.domain, "remembered": s.remembered, "saved": true, "path": path_of(&s.uri)}))
        .collect();
    // Cloud accounts (rclone).
    if crate::launch::installed("rclone") {
        let run = std::env::var_os("XDG_RUNTIME_DIR")
            .map(PathBuf::from)
            .unwrap_or_default();
        for r in crate::cloudremotes::remotes("rclone") {
            let dir = crate::cloudremotes::mount_dir(&run, &r.name);
            let path = if crate::cloudremotes::mounted(&dir) {
                dir.to_string_lossy().into_owned()
            } else {
                String::new()
            };
            out.push(json!({"name": r.name, "uri": format!("rclone:{}", r.name), "kind": r.kind, "what": crate::cloudremotes::provider_name(&r.kind), "rclone": r.name, "saved": true, "path": path}));
        }
    }
    // Online accounts' drives (mounted or not).
    let accounts = shares::accounts();
    for a in &accounts {
        if !saved.iter().any(|s| s.uri == bare(&a.uri)) {
            let path = if now.iter().any(|(u, _)| bare(u) == bare(&a.uri)) {
                shares::local_path("gio", &a.uri)
                    .map(|p| p.to_string_lossy().into_owned())
                    .unwrap_or_default()
            } else {
                String::new()
            };
            out.push(json!({"name": a.name, "uri": a.uri, "user": "", "domain": "", "remembered": false, "saved": false, "account": a.id, "path": path}));
        }
    }
    for (u, name) in &now {
        let b = bare(u);
        if !saved.iter().any(|s| s.uri == b) && !accounts.iter().any(|a| bare(&a.uri) == b) {
            let path = shares::local_path("gio", u)
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
            out.push(json!({"name": name, "uri": u, "user": "", "domain": "", "remembered": false, "saved": false, "path": path}));
        }
    }
    json!(out).to_string()
}

impl qobject::Ops {
    pub fn cleanup(mut self: Pin<&mut Self>, kind: &QString, root: &QString) -> i32 {
        use crate::cleanup;
        let kind = kind.to_string();
        let root = PathBuf::from(root.to_string());
        self.as_mut().rust_mut().next += 1;
        let id = self.next;
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.measuring.lock().unwrap().insert(id, stop.clone());
        let measuring = self.measuring.clone();
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            crate::finder::background();
            let send = |v: Value| {
                let r = v.to_string();
                let _ = thread.queue(move |mut obj| {
                    obj.as_mut().cleanup_found(id, QString::from(r.as_str()))
                });
            };
            let mut last = Instant::now();
            let mut tell = |n: u64| {
                if last.elapsed() > Duration::from_millis(250) {
                    last = Instant::now();
                    send(json!({"done": false, "scanned": n}));
                }
            };
            let result = match kind.as_str() {
                "duplicates" => {
                    json!({"groups": cleanup::duplicates(&root, 1 << 20, &stop, &mut tell)})
                }
                "caches" => {
                    json!({"files": cleanup::caches(&root, &crate::places::home(), &stop, &mut tell)})
                }
                _ => json!({"files": cleanup::largest(&root, 200, &stop, &mut tell)}),
            };
            if !stop.load(Ordering::Relaxed) {
                let mut r = result;
                // Each with its icon (a file's type's, a cache's folder's).
                let ic = whiterose_kit::icons::shared();
                let icon = |path: &str, dir: bool| -> String {
                    let name = Path::new(path)
                        .file_name()
                        .map(|n| n.to_string_lossy().into_owned())
                        .unwrap_or_default();
                    let p = if dir {
                        ic.lookup("folder", 24, 1)
                    } else {
                        ic.mime
                            .icon_names(ic.mime.type_of(&name))
                            .iter()
                            .find_map(|n| ic.lookup(n, 24, 1))
                    };
                    p.map(|p| p.to_string_lossy().into_owned())
                        .unwrap_or_default()
                };
                let dirs = kind == "caches";
                let fill = |files: &mut Value| {
                    if let Some(a) = files.as_array_mut() {
                        for f in a {
                            let path = f["path"].as_str().unwrap_or("").to_string();
                            f["icon"] = json!(icon(&path, dirs));
                        }
                    }
                };
                if let Some(fs) = r.get_mut("files") {
                    fill(fs);
                }
                if let Some(gs) = r.get_mut("groups").and_then(Value::as_array_mut) {
                    for g in gs {
                        fill(&mut g["files"]);
                    }
                }
                r["done"] = json!(true);
                send(r);
            }
            measuring.lock().unwrap().remove(&id);
        });
        id
    }

    pub fn connect(mut self: Pin<&mut Self>, request: &QString) -> i32 {
        use crate::shares;
        let req: Value = serde_json::from_str(&request.to_string()).unwrap_or(Value::Null);
        let text = |k: &str| req.get(k).and_then(Value::as_str).unwrap_or("").to_string();
        let flag = |k: &str| req.get(k).and_then(Value::as_bool).unwrap_or(false);
        let (address, mut user, domain, password) = (
            text("address"),
            text("user"),
            text("domain"),
            text("password"),
        );
        let (remember, save, accept) = (flag("remember"), flag("save"), flag("accept"));
        self.as_mut().rust_mut().next += 1;
        let id = self.next;
        // Stopped with stopCleanup (Cancel while connecting).
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.measuring.lock().unwrap().insert(id, stop.clone());
        let measuring = self.measuring.clone();
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let send = |v: Value| {
                measuring.lock().unwrap().remove(&id);
                let r = v.to_string();
                let _ = thread
                    .queue(move |mut obj| obj.as_mut().connected(id, QString::from(r.as_str())));
            };
            // A cloud account (rclone): opened as a folder.
            if let Some(name) = req.get("rclone").and_then(Value::as_str) {
                let run = std::env::var_os("XDG_RUNTIME_DIR")
                    .map(PathBuf::from)
                    .unwrap_or_default();
                let dir = crate::cloudremotes::mount_dir(&run, name);
                let v = match crate::cloudremotes::mount("rclone", name, &dir) {
                    Ok(()) => json!({"result": "ok", "path": dir, "uri": format!("rclone:{name}")}),
                    Err(e) => {
                        json!({"result": "failed", "error": e, "uri": format!("rclone:{name}")})
                    }
                };
                return send(v);
            }
            // An online account's drive: its own sign-in.
            if let Some(acc) = req.get("account").and_then(Value::as_str) {
                let a = shares::Account {
                    id: acc.to_string(),
                    name: String::new(),
                    uri: address.clone(),
                };
                let mut v = json!(shares::mount_account(&a));
                v["uri"] = json!(address);
                return send(v);
            }
            let a = match shares::parse(&address) {
                Ok(a) => a,
                Err(e) => return send(json!({"result": "failed", "error": e})),
            };
            if user.is_empty() {
                user = a.user.clone();
            }
            // The address without the user (that's kept separately).
            let uri = shares::Address {
                user: String::new(),
                ..a.clone()
            }
            .uri();
            let file = shares::servers_file();
            let mut list = shares::load(&file);
            // A saved password, if none was typed.
            let mut password = password;
            if password.is_empty() && list.iter().any(|s| s.uri == uri && s.remembered) {
                if let Ok(v) = whiterose_kit::daemon::request(
                    json!({"op": "share_secret_get", "args": {"id": shares::secret_id(&uri, &user)}}),
                    Duration::from_secs(5),
                ) {
                    password = v.as_str().unwrap_or("").to_string();
                }
            }
            let gio_uri = if user.is_empty() {
                uri.clone()
            } else {
                shares::Address {
                    user: user.clone(),
                    ..a.clone()
                }
                .uri()
            };
            let r = shares::mount(
                "gio",
                &gio_uri,
                &user,
                &domain,
                &password,
                accept,
                Duration::from_secs(60),
                &stop,
            );
            let mut v = json!(r);
            if let shares::Mounted::Ok { .. } = r {
                let mut stored = String::new();
                if remember && !password.is_empty() {
                    match whiterose_kit::daemon::request(
                        json!({"op": "share_secret_set", "args": {"id": shares::secret_id(&uri, &user), "value": password}}),
                        Duration::from_secs(5),
                    ) {
                        Ok(w) => stored = w["stored"].as_str().unwrap_or("").to_string(),
                        Err(e) => v["warning"] = json!(format!("The password wasn't saved: {e}")),
                    }
                }
                if save || list.iter().any(|s| s.uri == uri) {
                    let was = list
                        .iter()
                        .find(|s| s.uri == uri)
                        .map(|s| s.remembered)
                        .unwrap_or(false);
                    shares::remember(
                        &mut list,
                        shares::Server {
                            name: a.name(),
                            uri: uri.clone(),
                            user: user.clone(),
                            domain: domain.clone(),
                            remembered: was || !stored.is_empty(),
                        },
                    );
                    let _ = shares::save(&file, &list);
                }
                v["stored"] = json!(stored);
            }
            v["uri"] = json!(uri);
            v["name"] = json!(a.name());
            send(v);
        });
        id
    }

    pub fn disconnect(&self, uri: &QString) -> QString {
        let uri = uri.to_string();
        if let Some(name) = uri.strip_prefix("rclone:") {
            let run = std::env::var_os("XDG_RUNTIME_DIR")
                .map(PathBuf::from)
                .unwrap_or_default();
            return match crate::cloudremotes::unmount(&crate::cloudremotes::mount_dir(&run, name)) {
                Ok(()) => QString::from(""),
                Err(e) => QString::from(e.as_str()),
            };
        }
        match crate::shares::unmount("gio", &uri) {
            Ok(()) => QString::from(""),
            Err(e) => QString::from(e.as_str()),
        }
    }

    pub fn servers(&self) -> QString {
        QString::from(servers_json().as_str())
    }

    pub fn ask_servers(&self) {
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let r = servers_json();
            let _ =
                thread.queue(move |mut obj| obj.as_mut().servers_listed(QString::from(r.as_str())));
        });
    }

    pub fn cloud_providers(&self) -> QString {
        let p: Vec<Value> = crate::cloudremotes::PROVIDERS
            .iter()
            .map(|(k, n)| json!({"kind": k, "name": n}))
            .collect();
        q(json!({"rclone": crate::launch::installed("rclone"), "providers": p}))
    }

    pub fn cloud_setup(mut self: Pin<&mut Self>, request: &QString) -> i32 {
        let req: Value = serde_json::from_str(&request.to_string()).unwrap_or(Value::Null);
        self.as_mut().rust_mut().next += 1;
        let id = self.next;
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.measuring.lock().unwrap().insert(id, stop.clone());
        let measuring = self.measuring.clone();
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let send = |v: Value| {
                let r = v.to_string();
                let _ = thread.queue(move |mut obj| {
                    obj.as_mut().cloud_setup_step(id, QString::from(r.as_str()))
                });
            };
            let s = |k: &str| req.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            let params: Vec<(String, String)> = req
                .get("params")
                .and_then(Value::as_object)
                .map(|o| {
                    o.iter()
                        .filter_map(|(k, v)| {
                            v.as_str()
                                .filter(|v| !v.is_empty())
                                .map(|v| (k.clone(), v.to_string()))
                        })
                        .collect()
                })
                .unwrap_or_default();
            let (state, answer) = (s("state"), s("answer"));
            let cont = req
                .get("state")
                .is_some()
                .then_some((state.as_str(), answer.as_str()));
            let step = crate::cloudremotes::setup(
                "rclone",
                &s("name"),
                &s("kind"),
                &params,
                cont,
                &stop,
                &mut |n| send(json!({"note": n})),
            );
            measuring.lock().unwrap().remove(&id);
            send(json!(step));
        });
        id
    }

    pub fn ask_containers(&self) {
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let engine = crate::containers::engine(&crate::launch::installed).unwrap_or("");
            let v = if engine.is_empty() {
                json!({"engine": "", "containers": []})
            } else {
                match crate::containers::list(engine) {
                    Ok(c) => json!({"engine": engine, "containers": c}),
                    Err(e) => json!({"engine": engine, "error": e, "containers": []}),
                }
            };
            let r = v.to_string();
            let _ = thread
                .queue(move |mut obj| obj.as_mut().containers_found(QString::from(r.as_str())));
        });
    }

    pub fn container_do(mut self: Pin<&mut Self>, request: &QString) -> i32 {
        use crate::containers as c;
        let req: Value = serde_json::from_str(&request.to_string()).unwrap_or(Value::Null);
        self.as_mut().rust_mut().next += 1;
        let job = self.next;
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let s = |k: &str| req.get(k).and_then(Value::as_str).unwrap_or("").to_string();
            let r: Result<Value, String> = match c::engine(&crate::launch::installed) {
                None => Err("Neither Docker nor Podman is installed".into()),
                Some(engine) => match s("what").as_str() {
                    "compose" => c::compose(engine, &s("action"), Path::new(&s("workdir")))
                        .map(|_| json!({})),
                    "copy" => c::copy_out(engine, &s("id"), &s("path"), Path::new(&s("dest")))
                        .map(|p| json!({"path": p})),
                    what => c::act(engine, what, &s("id")).map(|_| json!({})),
                },
            };
            let v = match r {
                Ok(mut v) => {
                    v["ok"] = json!(true);
                    v
                }
                Err(e) => json!({"ok": false, "error": e}),
            };
            let out = v.to_string();
            let _ = thread.queue(move |mut obj| {
                obj.as_mut()
                    .container_done(job, QString::from(out.as_str()))
            });
        });
        job
    }

    pub fn container_ls(&self, id: &QString, path: &QString) -> QString {
        let Some(engine) = crate::containers::engine(&crate::launch::installed) else {
            return q(json!({"error": "Neither Docker nor Podman is installed"}));
        };
        match crate::containers::ls(engine, &id.to_string(), &path.to_string()) {
            Ok(e) => q(json!({"entries": e})),
            Err(e) => q(json!({"error": e})),
        }
    }

    pub fn container_command(&self, what: &QString, id: &QString) -> QString {
        let Some(engine) = crate::containers::engine(&crate::launch::installed) else {
            return QString::from("");
        };
        QString::from(
            crate::containers::terminal_command(engine, &what.to_string(), &id.to_string())
                .unwrap_or_default()
                .as_str(),
        )
    }

    pub fn set_encrypt(&self, on: bool) {
        let was = crate::secure::on();
        crate::secure::set_on(on);
        if was == on && !on {
            return;
        }
        std::thread::spawn(|| {
            crate::finder::background();
            let log = crate::log::file();
            for f in [
                crate::jump::file(),
                log.with_extension("log.1"),
                log,
                crate::search::index_file(),
            ] {
                if !crate::secure::convert(&f) {
                    crate::log::warn(&format!(
                        "couldn't rewrite {} (no key from the keyring?)",
                        f.display()
                    ));
                }
            }
        });
    }

    pub fn interrupted(&self) -> QString {
        let f = crate::journal::file();
        let mine = format!("-{}-", std::process::id());
        let v: Vec<Value> = crate::journal::load(&f)
            .into_iter()
            // (Not this run's own transfers, still going.)
            .filter(|e| !e.key.contains(&mine))
            .filter_map(|e| {
                let rem = crate::journal::remaining(&e);
                if rem.is_empty() {
                    crate::journal::finish(&f, &e.key);
                    return None;
                }
                Some(json!({"key": e.key, "kind": e.kind, "dest": e.dest, "started": e.started, "remaining": rem}))
            })
            .collect();
        q(json!(v))
    }

    pub fn resume_transfer(self: Pin<&mut Self>, key: &QString) -> i32 {
        let f = crate::journal::file();
        let key = key.to_string();
        let Some(e) = crate::journal::load(&f).into_iter().find(|e| e.key == key) else {
            return -1;
        };
        let rem = crate::journal::remaining(&e);
        crate::journal::clear_parts(&e);
        crate::journal::finish(&f, &key);
        if rem.is_empty() {
            return -1;
        }
        let Some(kind) = Kind::parse(&e.kind) else {
            return -1;
        };
        crate::log::info(&format!(
            "carrying on with an interrupted {} to {}",
            e.kind, e.dest
        ));
        let control = Control {
            merge_folders: true,
            ..Default::default()
        };
        self.launch(
            kind,
            &e.kind,
            &q(json!(rem)),
            &QString::from(e.dest.as_str()),
            control,
        )
    }

    pub fn forget_transfer(&self, key: &QString) {
        crate::journal::finish(&crate::journal::file(), &key.to_string());
    }

    pub fn export_settings(&self, path: &QString) -> QString {
        let settings = whiterose_kit::daemon::request(json!({"op": "all"}), Duration::from_secs(5))
            .unwrap_or(Value::Null);
        let v = crate::prefs::export(&settings, &crate::prefs::parts());
        let p = PathBuf::from(path.to_string());
        let r = std::fs::write(&p, serde_json::to_string_pretty(&v).unwrap_or_default())
            .map_err(|e| e.to_string());
        QString::from(r.err().unwrap_or_default().as_str())
    }

    pub fn import_settings(&self, path: &QString) -> QString {
        use crate::prefs;
        let r = (|| -> Result<Value, String> {
            let text = std::fs::read_to_string(path.to_string()).map_err(|e| e.to_string())?;
            let v: Value = serde_json::from_str(&text)
                .map_err(|_| "That isn't a Files settings export".to_string())?;
            let (settings, files) = prefs::read(&v)?;
            // What's there now, kept first.
            let now = whiterose_kit::daemon::request(json!({"op": "all"}), Duration::from_secs(5))
                .unwrap_or(Value::Null);
            let backup = prefs::export(&now, &prefs::parts());
            let _ = std::fs::create_dir_all(crate::session::dir());
            std::fs::write(
                prefs::backup_file(),
                serde_json::to_string_pretty(&backup).unwrap_or_default(),
            )
            .map_err(|e| e.to_string())?;
            let n = prefs::write_files(&files, &prefs::parts())?;
            let mut failed = vec![];
            for (k, val) in &settings {
                if let Err(e) = whiterose_kit::daemon::request(
                    json!({"op": "set", "key": k, "value": val}),
                    Duration::from_secs(5),
                ) {
                    failed.push(format!("{k}: {e}"));
                }
            }
            Ok(
                json!({"ok": true, "settings": settings.len() - failed.len(), "files": n, "failed": failed}),
            )
        })();
        q(r.unwrap_or_else(|e| json!({"ok": false, "error": e})))
    }

    pub fn ssh_hosts(&self) -> QString {
        let cfg =
            std::fs::read_to_string(crate::places::home().join(".ssh/config")).unwrap_or_default();
        q(json!(crate::shares::ssh_hosts(&cfg)))
    }

    pub fn forget_server(&self, uri: &QString) {
        use crate::shares;
        // A cloud account: closed, and taken out of rclone's config.
        if let Some(name) = uri.to_string().strip_prefix("rclone:") {
            let run = std::env::var_os("XDG_RUNTIME_DIR")
                .map(PathBuf::from)
                .unwrap_or_default();
            let dir = crate::cloudremotes::mount_dir(&run, name);
            if crate::cloudremotes::mounted(&dir) {
                let _ = crate::cloudremotes::unmount(&dir);
            }
            let _ = crate::cloudremotes::remove("rclone", name);
            return;
        }
        let file = shares::servers_file();
        let mut list = shares::load(&file);
        let uri = uri.to_string();
        if let Some(s) = list.iter().find(|s| s.uri == uri) {
            if s.remembered {
                let _ = whiterose_kit::daemon::request(
                    json!({"op": "share_secret_delete", "args": {"id": shares::secret_id(&s.uri, &s.user)}}),
                    Duration::from_secs(5),
                );
            }
        }
        list.retain(|s| s.uri != uri);
        let _ = shares::save(&file, &list);
    }

    pub fn compare(
        mut self: Pin<&mut Self>,
        left: &QString,
        right: &QString,
        content: bool,
    ) -> i32 {
        let (left, right) = (
            PathBuf::from(left.to_string()),
            PathBuf::from(right.to_string()),
        );
        self.as_mut().rust_mut().next += 1;
        let id = self.next;
        let stop = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.measuring.lock().unwrap().insert(id, stop.clone());
        let measuring = self.measuring.clone();
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            crate::finder::background();
            let send = |v: Value| {
                let r = v.to_string();
                let _ = thread
                    .queue(move |mut obj| obj.as_mut().compared(id, QString::from(r.as_str())));
            };
            let mut last = Instant::now();
            let entries = crate::compare::compare(&left, &right, content, &stop, &mut |n| {
                if last.elapsed() > Duration::from_millis(250) {
                    last = Instant::now();
                    send(json!({"done": false, "scanned": n}));
                }
            });
            if !stop.load(Ordering::Relaxed) {
                send(json!({"done": true, "entries": entries}));
            }
            measuring.lock().unwrap().remove(&id);
        });
        id
    }

    pub fn sync_plan(
        &self,
        left: &QString,
        right: &QString,
        entries: &QString,
        mode: &QString,
        towards: &QString,
    ) -> QString {
        let entries: Vec<crate::compare::Entry> =
            serde_json::from_str(&entries.to_string()).unwrap_or_default();
        let p = crate::compare::plan(
            Path::new(&left.to_string()),
            Path::new(&right.to_string()),
            &entries,
            &mode.to_string(),
            &towards.to_string(),
        );
        q(json!(p))
    }

    pub fn sync(self: Pin<&mut Self>, plan: &QString) -> i32 {
        let Ok(plan) = serde_json::from_str::<crate::compare::Plan>(&plan.to_string()) else {
            return -1;
        };
        if !plan.left.is_absolute() || !plan.right.is_absolute() {
            return -1;
        }
        let sources = q(json!([plan.left, plan.right]));
        let control = Control::default();
        *control.sync.lock().unwrap() = Some(plan);
        self.launch(Kind::Sync, "sync", &sources, &QString::from(""), control)
    }

    pub fn stop_cleanup(&self, id: i32) {
        if let Some(s) = self.measuring.lock().unwrap().remove(&id) {
            s.store(true, Ordering::Relaxed);
        }
    }

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

fn bulk_plan(
    paths: &QString,
    dirs: &QString,
    rules: &QString,
) -> Result<Vec<crate::bulk_rename::Outcome>, String> {
    let paths: Vec<String> = serde_json::from_str(&paths.to_string()).map_err(|e| e.to_string())?;
    let dirs: Vec<bool> = serde_json::from_str(&dirs.to_string()).unwrap_or_default();
    let rules: crate::bulk_rename::Rules =
        serde_json::from_str(&rules.to_string()).map_err(|e| e.to_string())?;
    crate::bulk_rename::preview(&paths, &dirs, &rules, &|p| {
        std::fs::symlink_metadata(p).is_ok()
    })
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
            "Moving 2 items to the trash"
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
