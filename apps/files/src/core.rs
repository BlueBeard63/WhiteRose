//! `Fs` (QML singleton): the file manager's Rust side for QML. Results come
//! back as JSON.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
    }

    unsafe extern "C++" {
        include!("whiterose-files/cpp/image.h");
        #[cxx_name = "imageSize"]
        fn image_size(path: &str) -> String;
    }

    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        type Fs = super::FsRust;

        /// {ok, entries: [{name, path, dir, link, size, modified, hidden,
        /// executable, mime, icon}], stamp} or {ok: false, error}.
        /// sort: name | size | modified | kind.
        #[qinvokable]
        fn list(
            self: &Fs,
            path: &QString,
            hidden: bool,
            sort: &QString,
            descending: bool,
            icon_size: i32,
            folders: &QString,
        ) -> QString;
        /// Changes when entries are added, removed or renamed (to re-list).
        #[qinvokable]
        fn stamp(self: &Fs, path: &QString) -> f64;
        /// {folders, bookmarks, drives}, each with an icon file.
        #[qinvokable]
        fn places(self: &Fs) -> QString;
        #[qinvokable]
        #[cxx_name = "setBookmark"]
        fn set_bookmark(self: &Fs, path: &QString, on: bool) -> bool;
        #[qinvokable]
        fn crumbs(self: &Fs, path: &QString) -> QString;
        #[qinvokable]
        fn suggest(self: &Fs, typed: &QString, hidden: bool) -> QString;
        /// Folders that fit a few letters, from anywhere (jump.rs): [{path, name}].
        #[qinvokable]
        fn jump(self: &Fs, text: &QString) -> QString;
        /// A folder was opened (for jump's ranking).
        #[qinvokable]
        fn visited(self: &Fs, path: &QString);
        /// The folders in `path`, for the sidebar's tree: [{name, path, more}]
        /// (`more`: it has folders in it).
        #[qinvokable]
        fn subfolders(self: &Fs, path: &QString, hidden: bool) -> QString;
        /// Puts a bookmark at position `to` (others keep their order).
        /// The apps that open `mime`, the default first: [{id, name, icon, default}].
        #[qinvokable]
        #[cxx_name = "appsFor"]
        fn apps_for(self: &Fs, mime: &QString) -> QString;
        /// Makes app `id` the default for `mime` (for every app: mimeapps.list).
        #[qinvokable]
        #[cxx_name = "setDefaultApp"]
        fn set_default_app(self: &Fs, mime: &QString, id: &QString) -> bool;
        /// Opens these files ([path]) with app `id`: "" or what went wrong.
        #[qinvokable]
        #[cxx_name = "openWith"]
        fn open_with(self: &Fs, id: &QString, paths: &QString) -> QString;
        /// A file's comment ("" if none).
        #[qinvokable]
        fn comment(self: &Fs, path: &QString) -> QString;
        /// "" or why not.
        #[qinvokable]
        #[cxx_name = "setComment"]
        fn set_comment(self: &Fs, path: &QString, text: &QString) -> QString;
        /// A JSON, JSONC or JSON5 file formatted: {ok, text} or {ok: false, error}.
        #[qinvokable]
        #[cxx_name = "prettyJson"]
        fn pretty_json(self: &Fs, path: &QString) -> QString;
        /// Saved workspaces: [{name, saved, windows, tabs, folders}].
        #[qinvokable]
        fn workspaces(self: &Fs) -> QString;
        /// Saves {windows} as `name`, with its folders' views: "" or why not.
        #[qinvokable]
        #[cxx_name = "saveWorkspace"]
        fn save_workspace(self: &Fs, name: &QString, json: &QString) -> QString;
        /// `name`'s views put back; {ok, windows} or {ok: false, error}.
        #[qinvokable]
        #[cxx_name = "openWorkspace"]
        fn open_workspace(self: &Fs, name: &QString) -> QString;
        #[qinvokable]
        #[cxx_name = "deleteWorkspace"]
        fn delete_workspace(self: &Fs, name: &QString) -> bool;
        /// "" or why not.
        #[qinvokable]
        #[cxx_name = "renameWorkspace"]
        fn rename_workspace(self: &Fs, from: &QString, to: &QString) -> QString;
        /// `path` has folders in it (the tree offers to open it).
        #[qinvokable]
        #[cxx_name = "hasSubfolders"]
        fn has_subfolders(self: &Fs, path: &QString, hidden: bool) -> bool;
        #[qinvokable]
        #[cxx_name = "moveBookmark"]
        fn move_bookmark(self: &Fs, path: &QString, to: i32) -> bool;
        /// The icon for a type of file at `size` (a folder's when `dir`), "" if none.
        #[qinvokable]
        #[cxx_name = "typeIcon"]
        fn type_icon(self: &Fs, mime: &QString, dir: bool, size: i32) -> QString;
        /// An icon-theme name's file ("" if none).
        #[qinvokable]
        fn icon(self: &Fs, name: &QString, size: i32) -> QString;
        #[qinvokable]
        fn home(self: &Fs) -> QString;
        /// A typed location made absolute (~ expanded), "" if it isn't a folder.
        #[qinvokable]
        fn resolve(self: &Fs, typed: &QString) -> QString;
        /// The folder above `path` ("trash:" from a trashed folder, "" at the top).
        #[qinvokable]
        fn up(self: &Fs, path: &QString) -> QString;
        /// Everything about one file or folder, for Properties: {name, path,
        /// dir, link, target, mime, kind icon, size, modified, accessed,
        /// created, mode, owner, group, mine, origin, deleted}.
        #[qinvokable]
        fn info(self: &Fs, path: &QString) -> QString;
        /// Changes permissions (mode bits); "" or what went wrong.
        #[qinvokable]
        #[cxx_name = "setMode"]
        fn set_mode(self: &Fs, path: &QString, mode: i32) -> QString;
        /// A folder's own colour and icon: {colour, symbol, image} (null: none).
        #[qinvokable]
        fn look(self: &Fs, path: &QString) -> QString;
        /// Gives a folder a colour, symbol or picture ({} back to plain).
        #[qinvokable]
        #[cxx_name = "setLook"]
        fn set_look(self: &Fs, path: &QString, json: &QString) -> bool;
        /// The colours and symbols offered: {colours, symbols}.
        #[qinvokable]
        #[cxx_name = "lookChoices"]
        fn look_choices(self: &Fs) -> QString;
        /// On the same drive (a drop between them moves rather than copies).
        #[qinvokable]
        #[cxx_name = "sameDrive"]
        fn same_drive(self: &Fs, a: &QString, b: &QString) -> bool;
        /// A tab dragged between windows was taken (`id` from the drag): the
        /// window it came from checks with `tabTaken` when its drag ends.
        /// (Which drop action a drag reports varies between processes and
        /// compositors, so it isn't relied on.)
        #[qinvokable]
        #[cxx_name = "takeTab"]
        fn take_tab(self: &Fs, id: &QString);
        #[qinvokable]
        #[cxx_name = "tabTaken"]
        fn tab_taken(self: &Fs, id: &QString) -> bool;
        /// The mounted drives for the Storage page: [{name, path, icon,
        /// total, free, removable}], icons at 64 px.
        #[qinvokable]
        fn storage(self: &Fs) -> QString;
        /// Tag (`on`) or untag these files ([path]) with `tag`:
        /// {failed: [[path, why]]}.
        #[qinvokable]
        #[cxx_name = "setTag"]
        fn set_tag(self: &Fs, paths: &QString, tag: &QString, on: bool) -> QString;
        /// The tags these files have between them: {tag: how many have it}.
        #[qinvokable]
        #[cxx_name = "tagsOn"]
        fn tags_on(self: &Fs, paths: &QString) -> QString;
        /// Every tag Files knows files have: {tag: how many files}.
        #[qinvokable]
        #[cxx_name = "tagCounts"]
        fn tag_counts(self: &Fs) -> QString;
        /// Renames a tag on every file that has it (`to` "": takes it off them):
        /// {ok, error?, changed}.
        #[qinvokable]
        #[cxx_name = "renameTag"]
        fn rename_tag(self: &Fs, from: &QString, to: &QString) -> QString;
        /// A tag's name checked: {ok, name} or {ok: false, error}.
        #[qinvokable]
        #[cxx_name = "checkTag"]
        fn check_tag(self: &Fs, name: &QString) -> QString;
        /// Would opening it run code? {kind: none | program | script |
        /// launcher, why: "" or why where it is is untrusted}.
        #[qinvokable]
        #[cxx_name = "runInfo"]
        fn run_info(self: &Fs, path: &QString, mime: &QString) -> QString;
        /// Runs a program or script (asked first, in Files): "" or what went wrong.
        #[qinvokable]
        #[cxx_name = "runProgram"]
        fn run_program(self: &Fs, path: &QString) -> QString;
        /// An archive Files can extract.
        #[qinvokable]
        #[cxx_name = "isArchive"]
        fn is_archive(self: &Fs, name: &QString) -> bool;
        /// A file inside an archive ("archive:…!/…") as a real file to open or
        /// preview (taken out into a cache): its path, or "" if it can't be.
        #[qinvokable]
        #[cxx_name = "archiveFile"]
        fn archive_file(self: &Fs, place: &QString) -> QString;
        /// The password for an archive, for looking inside (memory only).
        #[qinvokable]
        #[cxx_name = "setArchivePassword"]
        fn set_archive_password(self: &Fs, archive: &QString, password: &QString);
        /// Something is at `path` (a file, folder or link).
        #[qinvokable]
        fn exists(self: &Fs, path: &QString) -> bool;
        /// A line in Files' log (level: info | warn | error).
        #[qinvokable]
        fn log(self: &Fs, level: &QString, message: &QString);
        /// What Files is running on, its settings and the end of its log, to
        /// paste into a bug report.
        #[qinvokable]
        fn diagnostics(self: &Fs, settings: &QString) -> QString;
        #[qinvokable]
        #[cxx_name = "logFile"]
        fn log_file(self: &Fs) -> QString;
        /// Installed terminals / code editors: [{id, name}].
        #[qinvokable]
        fn terminals(self: &Fs) -> QString;
        #[qinvokable]
        fn editors(self: &Fs) -> QString;
        /// The editor that `chosen` (a setting, "" for any) comes to: {id, name} or null.
        #[qinvokable]
        fn editor(self: &Fs, chosen: &QString) -> QString;
        /// Opens `dir` in the chosen terminal (or the first installed); "" or what went wrong.
        #[qinvokable]
        #[cxx_name = "openTerminal"]
        fn open_terminal(self: &Fs, dir: &QString, chosen: &QString) -> QString;
        /// Opens `path` (a folder or file) in the chosen code editor.
        #[qinvokable]
        #[cxx_name = "openEditor"]
        fn open_editor(self: &Fs, path: &QString, chosen: &QString) -> QString;
        /// Runs one of your own actions' shell command in `dir`, on its own
        /// (it outlives Files): "" or why not.
        #[qinvokable]
        #[cxx_name = "runCommand"]
        fn run_command(self: &Fs, dir: &QString, command: &QString) -> QString;
        /// The shelf (shelf.rs): [{path, name, dir, icon}].
        #[qinvokable]
        fn shelf(self: &Fs) -> QString;
        /// Puts paths (JSON) on the shelf: how many went on.
        #[qinvokable]
        #[cxx_name = "shelfAdd"]
        fn shelf_add(self: &Fs, paths: &QString) -> i32;
        #[qinvokable]
        #[cxx_name = "shelfRemove"]
        fn shelf_remove(self: &Fs, paths: &QString);
        #[qinvokable]
        #[cxx_name = "shelfClear"]
        fn shelf_clear(self: &Fs);
        /// Docker or Podman is installed (the Containers place shows).
        #[qinvokable]
        #[cxx_name = "hasContainers"]
        fn has_containers(self: &Fs) -> bool;
        /// The kinds of project `dir` is and their scripts (projects.rs):
        /// {projects: [{kind, file, scripts: [{name, command}]}], why} (why:
        /// it's somewhere untrusted, so running asks first).
        #[qinvokable]
        fn projects(self: &Fs, dir: &QString) -> QString;
        /// Runs `command` in `dir` in your terminal (it stays open after):
        /// "" or what went wrong.
        #[qinvokable]
        #[cxx_name = "runInTerminal"]
        fn run_in_terminal(
            self: &Fs,
            dir: &QString,
            command: &QString,
            chosen: &QString,
        ) -> QString;
        /// Opens WhiteRose Settings at `page` ("files", "files/search", ...);
        /// "" or what went wrong.
        #[qinvokable]
        #[cxx_name = "openSettings"]
        fn open_settings(self: &Fs, page: &QString) -> QString;
        /// The git repository `path` is in ("" if none).
        #[qinvokable]
        #[cxx_name = "gitRoot"]
        fn git_root(self: &Fs, path: &QString) -> QString;
        /// Folders named on the command line, as JSON.
        #[qinvokable]
        fn arguments(self: &Fs) -> QString;
        /// Started as a picker (`--choose`, choose.rs): what's wanted, as
        /// JSON; "null" otherwise.
        #[qinvokable]
        fn choosing(self: &Fs) -> QString;
        /// True if `name` / `mime` is one of filter `filter`'s (its number
        /// in `choosing`'s filters).
        #[qinvokable]
        #[cxx_name = "chooseMatches"]
        fn choose_matches(self: &Fs, filter: i32, name: &QString, mime: &QString) -> bool;
        /// The picker's answer: `paths` (JSON) printed and exit 0, or
        /// (null) nothing and exit 1.
        #[qinvokable]
        #[cxx_name = "chooseDone"]
        fn choose_done(self: &Fs, paths: &QString);
        /// The windows and tabs saved last time, cleaned up ({windows: []} if none).
        #[qinvokable]
        #[cxx_name = "loadSession"]
        fn load_session(self: &Fs) -> QString;
        #[qinvokable]
        #[cxx_name = "saveSession"]
        fn save_session(self: &Fs, json: &QString) -> bool;

        /// How `path` is shown: {mode, sortBy, descending, own, subfolders, from}.
        #[qinvokable]
        fn view(self: &Fs, path: &QString, fallback: &QString) -> QString;
        /// Remembers {mode, sortBy, descending} for this folder.
        #[qinvokable]
        #[cxx_name = "setView"]
        fn set_view(self: &Fs, path: &QString, json: &QString) -> bool;
        /// This folder lends its view to its subfolders (or stops).
        #[qinvokable]
        #[cxx_name = "setViewForSubfolders"]
        fn set_view_for_subfolders(self: &Fs, path: &QString, on: bool) -> bool;
        #[qinvokable]
        #[cxx_name = "resetView"]
        fn reset_view(self: &Fs, path: &QString) -> bool;
        /// An image's size: "4032 × 3024" ("" if it can't be read).
        #[qinvokable]
        #[cxx_name = "imageSize"]
        fn image_dimensions(self: &Fs, path: &QString) -> QString;
        /// The start of a text file, to preview: {ok, text, more} or
        /// {ok: false} (not text: has NULs or isn't UTF-8).
        #[qinvokable]
        #[cxx_name = "readText"]
        fn read_text(self: &Fs, path: &QString, limit: i32) -> QString;
    }
}

use std::{
    os::unix::fs::MetadataExt,
    path::{Path, PathBuf},
};

use cxx_qt_lib::QString;
use serde_json::{json, Value};
use whiterose_kit::icons;

use crate::{launch, listing, looks, places, recent, search, session, tags, trash, views};

/// The Trash, as a place to go.
const TRASH: &str = "trash:";
/// A tag's files, wherever they are ("tag:Name").
const TAG: &str = "tag:";
/// Every drive, how full (the Storage page).
const DRIVES: &str = "drives:";
/// Files opened or changed lately (recent.rs).
const RECENT: &str = "recent:";
/// The start page (StartMode.qml draws it; nothing is listed).
const START: &str = "start:";
/// Clean up (CleanupMode.qml; cleanup.rs looks).
const CLEANUP: &str = "cleanup:";
/// Docker / Podman containers (ContainersMode.qml; containers.rs).
const CONTAINERS: &str = "containers:";

#[derive(Default)]
pub struct FsRust;

fn q(v: Value) -> QString {
    QString::from(v.to_string().as_str())
}

fn icon_path(p: Option<std::path::PathBuf>) -> String {
    p.map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

fn now_s() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Entries for files from all over (a tag's, Recent): each with the folder
/// it's in noted in `origins`.
fn scattered(
    paths: Vec<std::path::PathBuf>,
    origins: &mut std::collections::HashMap<String, String>,
    mime: &dyn Fn(&str) -> String,
) -> Vec<listing::Entry> {
    let ms = |t: std::io::Result<std::time::SystemTime>| {
        t.ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0)
    };
    paths
        .into_iter()
        .filter_map(|p| {
            let m = std::fs::metadata(&p).ok()?;
            let name = p.file_name()?.to_string_lossy().into_owned();
            let dir = m.is_dir();
            origins.insert(
                p.to_string_lossy().into_owned(),
                p.parent()
                    .map(|d| d.to_string_lossy().into_owned())
                    .unwrap_or_default(),
            );
            Some(listing::Entry {
                mime: if dir {
                    "inode/directory".into()
                } else {
                    mime(&name)
                },
                hidden: name.starts_with('.'),
                executable: false,
                size: if dir { 0 } else { m.len() },
                modified: ms(m.modified()),
                created: ms(m.created()),
                link: false,
                dir,
                path: p.to_string_lossy().into_owned(),
                name,
            })
        })
        .collect()
}

impl qobject::Fs {
    pub fn list(
        &self,
        path: &QString,
        hidden: bool,
        sort: &QString,
        descending: bool,
        icon_size: i32,
        folders: &QString,
    ) -> QString {
        let path = path.to_string();
        let ic = icons::shared();
        // The Trash ("trash:"): everything in every Trash, named and typed as
        // it was, dated when it was trashed, with where it came from.
        let mut origins = std::collections::HashMap::new();
        let read = if path == TRASH {
            Ok(trash::list()
                .into_iter()
                .filter_map(|t| {
                    let m = std::fs::symlink_metadata(&t.path).ok()?;
                    let name = t.origin.file_name()?.to_string_lossy().into_owned();
                    let p = t.path.to_string_lossy().into_owned();
                    let dir = m.is_dir();
                    origins.insert(
                        p.clone(),
                        t.origin
                            .parent()
                            .map(|d| d.to_string_lossy().into_owned())
                            .unwrap_or_default(),
                    );
                    Some(listing::Entry {
                        mime: if dir {
                            "inode/directory".into()
                        } else {
                            ic.mime.type_of(&name).to_string()
                        },
                        hidden: false,
                        executable: false,
                        size: if dir { 0 } else { m.len() },
                        modified: t.deleted,
                        created: 0,
                        link: m.file_type().is_symlink(),
                        dir,
                        path: p,
                        name,
                    })
                })
                .collect())
        } else if let Some(tag) = path.strip_prefix(TAG) {
            // A tag ("tag:Name"): every file with it, wherever it is.
            Ok(scattered(tags::files_with(tag), &mut origins, &|n| {
                ic.mime.type_of(n).to_string()
            }))
        } else if let Some((archive, inside)) = crate::archive::split_place(&path) {
            // Inside an archive: what's in that folder of it.
            crate::archive::list(&archive, &inside).map(|items| {
                items
                    .into_iter()
                    .map(|i| {
                        let name = i.path.rsplit('/').next().unwrap_or(&i.path).to_string();
                        listing::Entry {
                            mime: if i.dir {
                                "inode/directory".into()
                            } else {
                                ic.mime.type_of(&name).to_string()
                            },
                            hidden: name.starts_with('.'),
                            executable: false,
                            size: i.size,
                            modified: i.modified,
                            created: 0,
                            link: false,
                            dir: i.dir,
                            path: crate::archive::place(&archive, &i.path),
                            name,
                        }
                    })
                    .collect()
            })
        } else if path == START || path == CLEANUP || path == CONTAINERS {
            Ok(vec![])
        } else if path == RECENT {
            // Recent: opened lately, or changed in the last two weeks.
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);
            let xbel = std::fs::read_to_string(recent::xbel_file()).unwrap_or_default();
            let list = recent::files(&xbel, &search::index_file(), &places::home(), now);
            Ok(scattered(
                list.into_iter().map(|(p, _)| p).collect(),
                &mut origins,
                &|n| ic.mime.type_of(n).to_string(),
            ))
        } else {
            listing::read(Path::new(&path), hidden, |n| ic.mime.type_of(n).to_string())
        };
        let mut entries = match read {
            Ok(v) => v,
            Err(e) => return q(json!({"ok": false, "error": e})),
        };
        let by = match sort.to_string().as_str() {
            "size" => listing::Sort::Size,
            "modified" => listing::Sort::Modified,
            "kind" => listing::Sort::Kind,
            "created" => listing::Sort::Created,
            _ => listing::Sort::Name,
        };
        let folders = listing::Folders::parse(&folders.to_string());
        listing::sort_with(&mut entries, by, descending, folders);
        let size = icon_size.clamp(16, 256) as u32;
        let looks = looks::load();
        let out: Vec<Value> = entries
            .into_iter()
            .map(|e| {
                let icon = icon_path(if e.dir {
                    ic.lookup("folder", size, 1)
                } else {
                    ic.mime
                        .icon_names(&e.mime)
                        .iter()
                        .find_map(|n| ic.lookup(n, size, 1))
                });
                let mut v = serde_json::to_value(&e).unwrap_or_default();
                v["icon"] = json!(icon);
                if let Some(o) = origins.get(&e.path) {
                    v["origin"] = json!(o);
                }
                if let Some(l) = looks.get(&e.path).filter(|_| e.dir) {
                    v["look"] = json!(l);
                }
                let t = tags::read(Path::new(&e.path));
                if !t.is_empty() {
                    v["tags"] = json!(t);
                }
                v
            })
            .collect();
        q(json!({"ok": true, "entries": out, "stamp": self.stamp_of(&path)}))
    }

    fn stamp_of(&self, path: &str) -> f64 {
        if let Some((archive, _)) = crate::archive::split_place(path) {
            return self.stamp_of(&archive.to_string_lossy());
        }
        if path == RECENT {
            // A file opened (the shared list) or the index made again.
            return self.stamp_of(&recent::xbel_file().to_string_lossy())
                + self.stamp_of(&search::index_file().to_string_lossy());
        }
        if path.starts_with(TAG) {
            // Tagging or untagging anything changes the list.
            return self.stamp_of(&tags::file().to_string_lossy());
        }
        if path == TRASH {
            // Something trashed, restored or deleted changes an info folder.
            return trash::all_trashes()
                .iter()
                .map(|t| self.stamp_of(&t.join("info").to_string_lossy()))
                .sum();
        }
        // A folder's own modification time changes when its entries do.
        std::fs::metadata(path)
            .map(|m| m.mtime() as f64 * 1000.0 + m.mtime_nsec() as f64 / 1e6)
            .unwrap_or(0.0)
    }

    pub fn stamp(&self, path: &QString) -> f64 {
        self.stamp_of(&path.to_string())
    }

    pub fn places(&self) -> QString {
        let ic = icons::shared();
        let looks = looks::load();
        let with_icon = |p: places::Place| {
            let file = icon_path(
                ic.lookup(&p.icon, 20, 1)
                    .or_else(|| ic.lookup("folder", 20, 1)),
            );
            json!({"name": p.name, "path": p.path, "icon": file, "look": looks.get(&p.path)})
        };
        let drives: Vec<Value> = places::drives()
            .into_iter()
            .map(|d| {
                let file = icon_path(
                    ic.lookup(&d.icon, 20, 1)
                        .or_else(|| ic.lookup("drive-harddisk", 20, 1)),
                );
                let mut v = serde_json::to_value(&d).unwrap_or_default();
                v["icon"] = json!(file);
                v
            })
            .collect();
        q(json!({
            "folders": places::folders()
                .into_iter()
                .chain(std::iter::once(places::Place {
                    name: "Trash".into(),
                    path: TRASH.into(),
                    icon: if trash::list().is_empty() { "user-trash" } else { "user-trash-full" }.into(),
                }))
                .map(with_icon)
                .collect::<Vec<_>>(),
            "bookmarks": places::bookmarks().into_iter().map(with_icon).collect::<Vec<_>>(),
            "drives": drives,
        }))
    }

    pub fn set_bookmark(&self, path: &QString, on: bool) -> bool {
        places::set_bookmark(&path.to_string(), on).is_ok()
    }

    pub fn crumbs(&self, path: &QString) -> QString {
        let path = path.to_string();
        // Inside an archive: the folders down to it, the archive, then inside.
        if let Some((archive, inside)) = crate::archive::split_place(&path) {
            let dir = archive
                .parent()
                .map(|d| d.to_string_lossy().into_owned())
                .unwrap_or_default();
            let mut v: Vec<Value> =
                serde_json::from_str(&self.crumbs(&QString::from(dir.as_str())).to_string())
                    .unwrap_or_default();
            v.push(json!({"name": archive.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(), "path": crate::archive::place(&archive, "")}));
            let mut at = String::new();
            for part in inside.split('/').filter(|p| !p.is_empty()) {
                at = if at.is_empty() {
                    part.to_string()
                } else {
                    format!("{at}/{part}")
                };
                v.push(json!({"name": part, "path": crate::archive::place(&archive, &at)}));
            }
            return q(json!(v));
        }
        if path == DRIVES {
            return q(json!([{"name": "Storage", "path": DRIVES}]));
        }
        if path == RECENT {
            return q(json!([{"name": "Recent", "path": RECENT}]));
        }
        if path == START {
            return q(json!([{"name": "Start", "path": START}]));
        }
        if path == CONTAINERS {
            return q(json!([{"name": "Containers", "path": CONTAINERS}]));
        }
        if path == CLEANUP {
            return q(
                json!([{"name": "Storage", "path": DRIVES}, {"name": "Clean up", "path": CLEANUP}]),
            );
        }
        if let Some(tag) = path.strip_prefix(TAG) {
            return q(json!([{"name": "Tags", "path": ""}, {"name": tag, "path": path}]));
        }
        let trash_crumb = json!({"name": "Trash", "path": TRASH});
        if path == TRASH {
            return q(json!([trash_crumb]));
        }
        // In a trashed folder: Trash > it > ...
        if let Some((t, parts)) = trash::inside(Path::new(&path)) {
            let mut acc = t.join("files");
            let mut v = vec![trash_crumb];
            for part in parts {
                acc = acc.join(&part);
                let name = if v.len() == 1 {
                    trash::list_in(&t)
                        .into_iter()
                        .find(|x| x.path == acc)
                        .and_then(|x| {
                            x.origin
                                .file_name()
                                .map(|n| n.to_string_lossy().into_owned())
                        })
                        .unwrap_or(part)
                } else {
                    part
                };
                v.push(json!({"name": name, "path": acc}));
            }
            return q(json!(v));
        }
        let path = QString::from(path.as_str());
        q(
            serde_json::to_value(places::crumbs(&path.to_string(), &places::home()))
                .unwrap_or_default(),
        )
    }

    pub fn suggest(&self, typed: &QString, hidden: bool) -> QString {
        q(json!(places::suggest(
            &typed.to_string(),
            &places::home(),
            hidden
        )))
    }

    pub fn icon(&self, name: &QString, size: i32) -> QString {
        QString::from(
            icon_path(icons::shared().lookup(&name.to_string(), size.clamp(8, 512) as u32, 1))
                .as_str(),
        )
    }

    pub fn type_icon(&self, mime: &QString, dir: bool, size: i32) -> QString {
        let ic = icons::shared();
        let size = size.clamp(16, 512) as u32;
        let p = if dir {
            ic.lookup("folder", size, 1)
        } else {
            ic.mime
                .icon_names(&mime.to_string())
                .iter()
                .find_map(|n| ic.lookup(n, size, 1))
        };
        QString::from(icon_path(p).as_str())
    }

    pub fn home(&self) -> QString {
        QString::from(places::home().to_string_lossy().as_ref())
    }

    pub fn jump(&self, text: &QString) -> QString {
        let ix = search::Index::open(&search::index_file());
        q(json!(crate::jump::find(
            &text.to_string(),
            &crate::jump::load(&crate::jump::file()),
            ix.as_ref(),
            now_s(),
            8
        )))
    }

    pub fn visited(&self, path: &QString) {
        crate::jump::record(&crate::jump::file(), &path.to_string(), now_s());
    }

    pub fn subfolders(&self, path: &QString, hidden: bool) -> QString {
        q(json!(places::subfolders(
            Path::new(&path.to_string()),
            hidden
        )))
    }

    pub fn apps_for(&self, mime: &QString) -> QString {
        let ic = icons::shared();
        let held = crate::apps::cached();
        let mut v = crate::apps::for_mime(&mime.to_string(), &held.0, &held.1);
        for c in v.iter_mut() {
            // The icon's file (a theme name or a path).
            c.icon = if c.icon.starts_with('/') {
                c.icon.clone()
            } else {
                icon_path(
                    ic.lookup(&c.icon, 32, 1)
                        .or_else(|| ic.lookup("application-x-executable", 32, 1)),
                )
            };
        }
        q(json!(v))
    }

    pub fn set_default_app(&self, mime: &QString, id: &QString) -> bool {
        let file = crate::apps::list_files().remove(0);
        let ok = crate::apps::set_default(&file, &mime.to_string(), &id.to_string()).is_ok();
        crate::apps::forget();
        ok
    }

    pub fn open_with(&self, id: &QString, paths: &QString) -> QString {
        let paths: Vec<String> = serde_json::from_str(&paths.to_string()).unwrap_or_default();
        let term = launch::pick(launch::TERMINALS, "", launch::installed).map(|t| t.id);
        match crate::apps::open_with(&id.to_string(), &paths, term) {
            Ok(()) => QString::from(""),
            Err(e) => QString::from(e.as_str()),
        }
    }

    pub fn comment(&self, path: &QString) -> QString {
        QString::from(tags::comment(Path::new(&path.to_string())).as_str())
    }

    pub fn set_comment(&self, path: &QString, text: &QString) -> QString {
        match tags::set_comment(Path::new(&path.to_string()), &text.to_string()) {
            Ok(()) => QString::from(""),
            Err(e) => QString::from(e.as_str()),
        }
    }

    pub fn pretty_json(&self, path: &QString) -> QString {
        let p = path.to_string();
        let text = match std::fs::metadata(&p) {
            Ok(m) if m.len() > 8 << 20 => {
                return q(json!({"ok": false, "error": "Too big to format (over 8 MB)"}))
            }
            Ok(_) => std::fs::read_to_string(&p).map_err(|e| e.to_string()),
            Err(e) => Err(e.to_string()),
        };
        match text.and_then(|t| crate::json_view::pretty(&t)) {
            Ok(t) => q(json!({"ok": true, "text": t})),
            Err(e) => q(json!({"ok": false, "error": e})),
        }
    }

    pub fn workspaces(&self) -> QString {
        q(json!(crate::workspaces::list(&crate::workspaces::file())))
    }

    pub fn save_workspace(&self, name: &QString, json: &QString) -> QString {
        let s: session::Session = serde_json::from_str(&json.to_string()).unwrap_or_default();
        let all = views::load_from(&views::file());
        match crate::workspaces::save(
            &crate::workspaces::file(),
            &all,
            &name.to_string(),
            s,
            now_s(),
        ) {
            Ok(()) => QString::from(""),
            Err(e) => QString::from(e.as_str()),
        }
    }

    pub fn open_workspace(&self, name: &QString) -> QString {
        let mut wanted: Vec<(String, views::Folder)> = vec![];
        let r = crate::workspaces::open(
            &crate::workspaces::file(),
            &name.to_string(),
            &places::home(),
            &mut |p, f| wanted.push((p.to_string(), f.clone())),
        );
        match r {
            Ok(s) => {
                if !wanted.is_empty() {
                    views::change(|all| {
                        for (p, f) in wanted {
                            all.set(&p, f.view.clone());
                            all.set_subfolders(&p, f.subfolders);
                        }
                    });
                }
                q(json!({"ok": true, "windows": s.windows}))
            }
            Err(e) => q(json!({"ok": false, "error": e})),
        }
    }

    pub fn delete_workspace(&self, name: &QString) -> bool {
        crate::workspaces::delete(&crate::workspaces::file(), &name.to_string()).is_ok()
    }

    pub fn rename_workspace(&self, from: &QString, to: &QString) -> QString {
        match crate::workspaces::rename(
            &crate::workspaces::file(),
            &from.to_string(),
            &to.to_string(),
        ) {
            Ok(()) => QString::from(""),
            Err(e) => QString::from(e.as_str()),
        }
    }

    pub fn has_subfolders(&self, path: &QString, hidden: bool) -> bool {
        places::has_subfolders(Path::new(&path.to_string()), hidden)
    }

    pub fn move_bookmark(&self, path: &QString, to: i32) -> bool {
        places::move_bookmark(&path.to_string(), to.max(0) as usize).is_ok()
    }

    pub fn resolve(&self, typed: &QString) -> QString {
        let t = typed.to_string();
        let t = t.trim().trim_start_matches("file://");
        if t.trim_end_matches('/') == TRASH {
            return QString::from(TRASH);
        }
        if t.trim_end_matches('/') == DRIVES {
            return QString::from(DRIVES);
        }
        for special in [RECENT, START, CLEANUP, CONTAINERS] {
            if t.trim_end_matches('/') == special {
                return QString::from(special);
            }
        }
        let p = match t.strip_prefix('~') {
            Some(rest) => format!("{}{rest}", places::home().to_string_lossy()),
            None => t.to_string(),
        };
        let p = Path::new(&p);
        if p.is_absolute() && p.is_dir() {
            QString::from(
                p.to_string_lossy()
                    .trim_end_matches('/')
                    .to_string()
                    .as_str(),
            )
        } else if p == Path::new("/") {
            QString::from("/")
        } else {
            QString::from("")
        }
    }

    pub fn choosing(&self) -> QString {
        q(choose_options().map_or(Value::Null, |c| serde_json::to_value(c).unwrap_or_default()))
    }

    pub fn choose_matches(&self, filter: i32, name: &QString, mime: &QString) -> bool {
        let Some(c) = choose_options() else {
            return true;
        };
        match c.filters.get(filter.max(0) as usize) {
            Some(f) => crate::choose::matches(f, &name.to_string(), &mime.to_string()),
            None => true,
        }
    }

    pub fn choose_done(&self, paths: &QString) {
        use std::io::Write;
        let json = choose_options().is_some_and(|c| c.json);
        match serde_json::from_str::<Vec<String>>(&paths.to_string()) {
            Ok(p) if !p.is_empty() => {
                let mut out = std::io::stdout().lock();
                let _ = out.write_all(crate::choose::output(&p, json).as_bytes());
                let _ = out.flush();
                std::process::exit(0)
            }
            _ => std::process::exit(1),
        }
    }

    pub fn arguments(&self) -> QString {
        // (A picker's options aren't folders to open.)
        if choose_options().is_some() {
            return q(json!([]));
        }
        let cwd = std::env::current_dir().unwrap_or_default();
        let dirs: Vec<String> = std::env::args()
            .skip(1)
            .filter(|a| !a.starts_with('-'))
            .map(|a| {
                if a.starts_with(TRASH) {
                    return PathBuf::from(TRASH);
                }
                if a.starts_with(DRIVES) {
                    return PathBuf::from(DRIVES);
                }
                if a.starts_with(RECENT) {
                    return PathBuf::from(RECENT);
                }
                if a.starts_with(START) {
                    return PathBuf::from(START);
                }
                if a.starts_with(CLEANUP) {
                    return PathBuf::from(CLEANUP);
                }
                if a.starts_with(CONTAINERS) {
                    return PathBuf::from(CONTAINERS);
                }
                if a.starts_with(crate::archive::PREFIX) {
                    return PathBuf::from(&a);
                }
                if a.starts_with(TAG) {
                    return PathBuf::from(&a);
                }
                let p = Path::new(a.strip_prefix("file://").unwrap_or(&a)).to_path_buf();
                let p = if p.is_absolute() { p } else { cwd.join(p) };
                // A file: open its folder.
                if p.is_dir() {
                    p
                } else {
                    p.parent().map(Path::to_path_buf).unwrap_or(p)
                }
            })
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        q(json!(dirs))
    }

    pub fn load_session(&self) -> QString {
        q(
            serde_json::to_value(session::load_from(&session::dir(), &places::home()))
                .unwrap_or_default(),
        )
    }

    pub fn save_session(&self, json: &QString) -> bool {
        match serde_json::from_str::<session::Session>(&json.to_string()) {
            Ok(s) => session::save_to(&session::dir(), &s).is_ok(),
            Err(_) => false,
        }
    }

    pub fn view(&self, path: &QString, fallback: &QString) -> QString {
        let mut found = views::load_from(&views::file()).find(&path.to_string());
        // Nothing of its own or lent: the default (whiterosed's files.defaultView).
        if !found.own && found.from.is_empty() {
            if let Ok(v) = serde_json::from_str::<views::View>(&fallback.to_string()) {
                found.view = v.checked();
            }
        }
        q(serde_json::to_value(found).unwrap_or_default())
    }

    pub fn set_view(&self, path: &QString, json: &QString) -> bool {
        let Ok(v) = serde_json::from_str::<views::View>(&json.to_string()) else {
            return false;
        };
        views::change(|all| all.set(&path.to_string(), v))
    }

    pub fn set_view_for_subfolders(&self, path: &QString, on: bool) -> bool {
        views::change(|all| all.set_subfolders(&path.to_string(), on))
    }

    pub fn reset_view(&self, path: &QString) -> bool {
        views::change(|all| all.reset(&path.to_string()))
    }

    pub fn image_dimensions(&self, path: &QString) -> QString {
        QString::from(qobject::image_size(&path.to_string()).as_str())
    }

    pub fn read_text(&self, path: &QString, limit: i32) -> QString {
        match crate::listing::text_start(
            Path::new(&path.to_string()),
            limit.clamp(1024, 1 << 20) as usize,
        ) {
            Some((text, more)) => q(json!({"ok": true, "text": text, "more": more})),
            None => q(json!({"ok": false})),
        }
    }

    pub fn up(&self, path: &QString) -> QString {
        let path = path.to_string();
        if let Some((archive, inside)) = crate::archive::split_place(&path) {
            return QString::from(
                if inside.is_empty() {
                    archive
                        .parent()
                        .map(|d| d.to_string_lossy().into_owned())
                        .unwrap_or_default()
                } else {
                    crate::archive::place(
                        &archive,
                        inside.rsplit_once('/').map(|(p, _)| p).unwrap_or(""),
                    )
                }
                .as_str(),
            );
        }
        if path == TRASH
            || path == DRIVES
            || path == RECENT
            || path == START
            || path == "/"
            || path.starts_with(TAG)
        {
            return QString::from("");
        }
        let p = Path::new(&path);
        if let Some((_, parts)) = trash::inside(p) {
            if parts.len() == 1 {
                return QString::from(TRASH);
            }
        }
        let up = p
            .parent()
            .map(|u| u.to_string_lossy().into_owned())
            .unwrap_or_default();
        QString::from(if up.is_empty() { "/" } else { up.as_str() })
    }

    pub fn info(&self, path: &QString) -> QString {
        let p = PathBuf::from(path.to_string());
        let Ok(lm) = std::fs::symlink_metadata(&p) else {
            return q(json!({"ok": false, "error": "it's not there any more"}));
        };
        let link = lm.file_type().is_symlink();
        let m = std::fs::metadata(&p).unwrap_or(lm.clone());
        let ms = |t: std::io::Result<std::time::SystemTime>| {
            t.ok()
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as i64)
                .unwrap_or(0)
        };
        let ic = icons::shared();
        // A trashed item: its own name, and where it came from.
        let trashed = trash::list().into_iter().find(|t| t.path == p);
        let name = trashed
            .as_ref()
            .and_then(|t| t.origin.file_name())
            .or(p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "/".into());
        let dir = m.is_dir();
        let mime = if dir {
            "inode/directory".to_string()
        } else {
            ic.mime.type_of(&name).to_string()
        };
        let icon = icon_path(if dir {
            ic.lookup("folder", 64, 1)
        } else {
            ic.mime
                .icon_names(&mime)
                .iter()
                .find_map(|n| ic.lookup(n, 64, 1))
        });
        // SAFETY: getuid can't fail.
        let me = unsafe { libc::getuid() };
        q(json!({
            "ok": true,
            "name": name,
            "path": p,
            "location": p.parent(),
            "dir": dir,
            "link": link,
            "target": if link { std::fs::read_link(&p).ok() } else { None },
            "mime": mime,
            "icon": icon,
            "size": if dir { 0 } else { m.len() },
            "modified": ms(lm.modified()),
            "accessed": ms(lm.accessed()),
            "created": ms(lm.created()),
            "mode": lm.mode() & 0o7777,
            "owner": user_name(lm.uid()),
            "group": group_name(lm.gid()),
            "mine": lm.uid() == me,
            "origin": trashed.as_ref().map(|t| t.origin.clone()),
            "deleted": trashed.map(|t| t.deleted),
        }))
    }

    pub fn set_mode(&self, path: &QString, mode: i32) -> QString {
        use std::os::unix::fs::PermissionsExt;
        let mode = (mode as u32) & 0o7777;
        match std::fs::set_permissions(path.to_string(), std::fs::Permissions::from_mode(mode)) {
            Ok(()) => QString::from(""),
            Err(e) => QString::from(e.to_string().as_str()),
        }
    }

    pub fn look(&self, path: &QString) -> QString {
        q(json!(looks::load().get(&path.to_string())))
    }

    pub fn set_look(&self, path: &QString, json: &QString) -> bool {
        let Ok(l) = serde_json::from_str::<looks::Look>(&json.to_string()) else {
            return false;
        };
        let path = path.to_string();
        looks::change(|all| {
            all.set(&path, l);
            true
        })
    }

    pub fn look_choices(&self) -> QString {
        q(json!({"colours": looks::COLOURS, "symbols": looks::SYMBOLS}))
    }

    pub fn same_drive(&self, a: &QString, b: &QString) -> bool {
        match (
            std::fs::symlink_metadata(a.to_string()),
            std::fs::metadata(b.to_string()),
        ) {
            (Ok(x), Ok(y)) => x.dev() == y.dev(),
            _ => false,
        }
    }

    pub fn take_tab(&self, id: &QString) {
        if let Some(p) = taken_file(&id.to_string()) {
            let _ = std::fs::create_dir_all(p.parent().unwrap_or(Path::new("/")));
            let _ = std::fs::write(p, "");
        }
    }

    pub fn tab_taken(&self, id: &QString) -> bool {
        taken_file(&id.to_string()).is_some_and(|p| std::fs::remove_file(p).is_ok())
    }

    pub fn storage(&self) -> QString {
        let ic = icons::shared();
        let v: Vec<Value> = places::drives()
            .into_iter()
            .map(|d| {
                let icon = icon_path(
                    ic.lookup(&d.icon, 64, 1)
                        .or_else(|| ic.lookup("drive-harddisk", 64, 1)),
                );
                let mut v = serde_json::to_value(&d).unwrap_or_default();
                v["icon"] = json!(icon);
                v
            })
            .collect();
        q(json!(v))
    }

    pub fn set_tag(&self, paths: &QString, tag: &QString, on: bool) -> QString {
        let paths: Vec<String> = serde_json::from_str(&paths.to_string()).unwrap_or_default();
        let Ok(tag) = tags::check_name(&tag.to_string()) else {
            return q(json!({"failed": [["", "not a tag name"]]}));
        };
        let mut failed = vec![];
        let mut done = vec![];
        for p in paths {
            let pb = PathBuf::from(&p);
            let mut t = tags::read(&pb);
            let has = t.contains(&tag);
            if on == has {
                done.push((p, t));
                continue;
            }
            if on {
                t.push(tag.clone());
            } else {
                t.retain(|x| *x != tag);
            }
            match tags::write(&pb, &t) {
                Ok(()) => done.push((p, t)),
                Err(e) => failed.push((p, e)),
            }
        }
        tags::change(|r| {
            for (p, t) in &done {
                r.set(p, t.clone());
            }
            true
        });
        q(json!({"failed": failed}))
    }

    pub fn tags_on(&self, paths: &QString) -> QString {
        let paths: Vec<PathBuf> = serde_json::from_str::<Vec<String>>(&paths.to_string())
            .unwrap_or_default()
            .into_iter()
            .map(PathBuf::from)
            .collect();
        q(json!(tags::on(&paths)))
    }

    pub fn tag_counts(&self) -> QString {
        q(json!(tags::counts()))
    }

    pub fn rename_tag(&self, from: &QString, to: &QString) -> QString {
        let from = from.to_string();
        let to = to.to_string();
        let to = if to.trim().is_empty() {
            None
        } else {
            match tags::check_name(&to) {
                Ok(n) => Some(n),
                Err(e) => return q(json!({"ok": false, "error": e})),
            }
        };
        // Files the list doesn't know of yet are picked up by the next index.
        let mut changed = vec![];
        tags::change(|r| {
            changed = r.rename(&from, to.as_deref());
            true
        });
        let mut failed = 0;
        for (p, t) in &changed {
            if tags::write(Path::new(p), t).is_err() {
                failed += 1;
            }
        }
        q(
            json!({"ok": failed == 0, "changed": changed.len(), "error": if failed > 0 { format!("{failed} files couldn't be changed") } else { String::new() }}),
        )
    }

    pub fn check_tag(&self, name: &QString) -> QString {
        match tags::check_name(&name.to_string()) {
            Ok(n) => q(json!({"ok": true, "name": n})),
            Err(e) => q(json!({"ok": false, "error": e})),
        }
    }

    pub fn run_info(&self, path: &QString, mime: &QString) -> QString {
        let p = PathBuf::from(path.to_string());
        let kind = crate::run::kind_of(&p, &mime.to_string());
        let why = if kind == crate::run::Kind::None {
            String::new()
        } else {
            untrusted_why(&p)
        };
        q(json!({"kind": kind, "why": why}))
    }

    pub fn run_command(&self, dir: &QString, command: &QString) -> QString {
        let (dir, command) = (dir.to_string(), command.to_string());
        crate::log::info(&format!("ran your action `{command}` in {dir}"));
        let r = launch::spawn("sh", &["-c".to_string(), command], &dir);
        QString::from(r.err().unwrap_or_default().as_str())
    }

    pub fn shelf(&self) -> QString {
        let ic = whiterose_kit::icons::shared();
        let v: Vec<Value> = crate::shelf::load(&crate::shelf::file())
            .into_iter()
            .map(|p| {
                let path = Path::new(&p);
                let name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| p.clone());
                let dir = path.is_dir();
                let icon = if dir {
                    ic.lookup("folder", 48, 1)
                } else {
                    ic.mime.icon_names(ic.mime.type_of(&name)).iter().find_map(|n| ic.lookup(n, 48, 1))
                };
                json!({"path": p, "name": name, "dir": dir, "icon": icon.map(|i| i.to_string_lossy().into_owned()).unwrap_or_default()})
            })
            .collect();
        q(json!(v))
    }

    pub fn shelf_add(&self, paths: &QString) -> i32 {
        let paths: Vec<String> = serde_json::from_str(&paths.to_string()).unwrap_or_default();
        crate::shelf::add(&crate::shelf::file(), &paths)
            .map(|n| n as i32)
            .unwrap_or(0)
    }

    pub fn shelf_remove(&self, paths: &QString) {
        let paths: Vec<String> = serde_json::from_str(&paths.to_string()).unwrap_or_default();
        let _ = crate::shelf::remove(&crate::shelf::file(), &paths);
    }

    pub fn shelf_clear(&self) {
        let _ = crate::shelf::clear(&crate::shelf::file());
    }

    pub fn has_containers(&self) -> bool {
        crate::containers::engine(&launch::installed).is_some()
    }

    pub fn projects(&self, dir: &QString) -> QString {
        let d = dir.to_string();
        if !d.starts_with('/') {
            return q(json!({"projects": [], "why": ""}));
        }
        let p = crate::projects::detect(Path::new(&d));
        let why = if p.is_empty() {
            String::new()
        } else {
            untrusted_why(&d)
        };
        q(json!({"projects": p, "why": why}))
    }

    pub fn run_in_terminal(&self, dir: &QString, command: &QString, chosen: &QString) -> QString {
        let (dir, command) = (dir.to_string(), command.to_string());
        let Some(t) = launch::pick(launch::TERMINALS, &chosen.to_string(), launch::installed)
        else {
            return QString::from("no terminal is installed");
        };
        crate::log::info(&format!("ran `{command}` in {dir}"));
        let args =
            crate::projects::terminal_run_args(t.id, &dir, &crate::projects::wrapped(&command));
        QString::from(
            launch::spawn(t.id, &args, &dir)
                .err()
                .unwrap_or_default()
                .as_str(),
        )
    }

    pub fn run_program(&self, path: &QString) -> QString {
        let p = path.to_string();
        let dir = Path::new(&p)
            .parent()
            .map(|d| d.to_string_lossy().into_owned())
            .unwrap_or_default();
        crate::log::info(&format!("ran {p} (asked first)"));
        let r = launch::spawn(&p, &[], &dir);
        if let Err(e) = &r {
            crate::log::warn(&format!("couldn't run {p}: {e}"));
        }
        QString::from(r.err().unwrap_or_default().as_str())
    }

    pub fn archive_file(&self, place: &QString) -> QString {
        let Some((archive, inside)) = crate::archive::split_place(&place.to_string()) else {
            return QString::from("");
        };
        match crate::archive::cached_file(&archive, &inside) {
            Ok(p) => QString::from(p.to_string_lossy().as_ref()),
            Err(e) => {
                crate::log::warn(&format!(
                    "taking {inside} out of {}: {e}",
                    archive.display()
                ));
                QString::from("")
            }
        }
    }

    pub fn set_archive_password(&self, archive: &QString, password: &QString) {
        crate::archive::set_password(Path::new(&archive.to_string()), &password.to_string());
    }

    pub fn exists(&self, path: &QString) -> bool {
        std::fs::symlink_metadata(path.to_string()).is_ok()
    }

    pub fn is_archive(&self, name: &QString) -> bool {
        crate::archive::format_of(&name.to_string()).is_some()
    }

    pub fn log(&self, level: &QString, message: &QString) {
        let m = message.to_string();
        match level.to_string().as_str() {
            "error" => crate::log::error(&m),
            "warn" => crate::log::warn(&m),
            _ => crate::log::info(&m),
        }
    }

    pub fn log_file(&self) -> QString {
        QString::from(crate::log::file().to_string_lossy().as_ref())
    }

    pub fn diagnostics(&self, settings: &QString) -> QString {
        let os = std::fs::read_to_string("/etc/os-release")
            .unwrap_or_default()
            .lines()
            .find_map(|l| {
                l.strip_prefix("PRETTY_NAME=")
                    .map(|v| v.trim_matches('"').to_string())
            })
            .unwrap_or_default();
        let env = |k: &str| std::env::var(k).unwrap_or_default();
        let index = search::index_file();
        let tools: Vec<String> = [
            "git",
            "udisksctl",
            "lsblk",
            "whiterose-notepad",
            "whiterose-settings",
        ]
        .iter()
        .map(|t| format!("{t}: {}", if launch::installed(t) { "yes" } else { "no" }))
        .collect();
        let mut t = String::new();
        t.push_str(&format!(
            "WhiteRose Files {}
",
            env!("CARGO_PKG_VERSION")
        ));
        t.push_str(&format!(
            "System: {os}; desktop {}; {}
",
            env("XDG_CURRENT_DESKTOP"),
            env("XDG_SESSION_TYPE")
        ));
        t.push_str(&format!(
            "Search index: {} bytes, made {}
",
            std::fs::metadata(&index).map(|m| m.len()).unwrap_or(0),
            crate::log::stamp_of(search::index_age(&index))
        ));
        t.push_str(&format!(
            "Tools: {}
",
            tools.join(", ")
        ));
        t.push_str(&format!(
            "Tagged files: {}
",
            tags::load().files.len()
        ));
        t.push_str(&format!(
            "Settings: {}
",
            settings
        ));
        t.push_str(
            "
Log (last 60 lines):
",
        );
        for l in crate::log::tail_of(&crate::log::file(), 60) {
            t.push_str(&l);
            t.push('\n');
        }
        QString::from(t.as_str())
    }

    pub fn terminals(&self) -> QString {
        q(json!(launch::available(
            launch::TERMINALS,
            launch::installed
        )))
    }

    pub fn editors(&self) -> QString {
        q(json!(launch::available(launch::EDITORS, launch::installed)))
    }

    pub fn editor(&self, chosen: &QString) -> QString {
        q(json!(launch::pick(
            launch::EDITORS,
            &chosen.to_string(),
            launch::installed
        )))
    }

    pub fn open_terminal(&self, dir: &QString, chosen: &QString) -> QString {
        let dir = dir.to_string();
        let Some(t) = launch::pick(launch::TERMINALS, &chosen.to_string(), launch::installed)
        else {
            return QString::from("no terminal is installed");
        };
        let r = launch::spawn(t.id, &launch::terminal_args(t.id, &dir), &dir);
        QString::from(r.err().unwrap_or_default().as_str())
    }

    pub fn open_editor(&self, path: &QString, chosen: &QString) -> QString {
        let path = path.to_string();
        let Some(e) = launch::pick(launch::EDITORS, &chosen.to_string(), launch::installed) else {
            return QString::from("no code editor is installed");
        };
        let dir = if Path::new(&path).is_dir() {
            path.clone()
        } else {
            Path::new(&path)
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default()
        };
        let r = launch::spawn(e.id, &[path], &dir);
        QString::from(r.err().unwrap_or_default().as_str())
    }

    pub fn open_settings(&self, page: &QString) -> QString {
        let page = page.to_string();
        if !page
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '/' || c == '-')
        {
            return QString::from("not a page");
        }
        if !launch::installed("whiterose-settings") {
            return QString::from("WhiteRose Settings isn't installed");
        }
        let home = places::home().to_string_lossy().into_owned();
        let r = launch::spawn("whiterose-settings", &[page], &home);
        QString::from(r.err().unwrap_or_default().as_str())
    }

    pub fn git_root(&self, path: &QString) -> QString {
        let p = launch::git_root(Path::new(&path.to_string()));
        QString::from(
            p.map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default()
                .as_str(),
        )
    }
}

/// $XDG_RUNTIME_DIR/whiterose/files-tabs/<id> (ids are letters and digits only).
fn taken_file(id: &str) -> Option<PathBuf> {
    if id.is_empty() || id.len() > 64 || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }
    let run = std::env::var_os("XDG_RUNTIME_DIR").map(PathBuf::from)?;
    Some(run.join("whiterose/files-tabs").join(id))
}

fn user_name(uid: u32) -> String {
    // SAFETY: getpwuid's result is read straight away, on this thread.
    unsafe {
        let pw = libc::getpwuid(uid);
        if pw.is_null() {
            return uid.to_string();
        }
        std::ffi::CStr::from_ptr((*pw).pw_name)
            .to_string_lossy()
            .into_owned()
    }
}

fn group_name(gid: u32) -> String {
    // SAFETY: as above.
    unsafe {
        let gr = libc::getgrgid(gid);
        if gr.is_null() {
            return gid.to_string();
        }
        std::ffi::CStr::from_ptr((*gr).gr_name)
            .to_string_lossy()
            .into_owned()
    }
}

/// Why `p` is somewhere untrusted (run.rs), "" if it isn't.
fn untrusted_why(p: impl AsRef<Path>) -> String {
    let home = places::home();
    let downloads = places::user_dirs(
        &std::fs::read_to_string(places::config_dir().join("user-dirs.dirs")).unwrap_or_default(),
        &home,
    )
    .into_iter()
    .find(|(k, _)| k == "DOWNLOAD")
    .map(|(_, p)| p)
    .unwrap_or_else(|| home.join("Downloads"));
    let removable: Vec<String> = places::drives()
        .into_iter()
        .filter(|d| d.removable)
        .map(|d| d.path)
        .collect();
    crate::run::untrusted(p.as_ref(), &home, Some(&downloads), &removable)
}

/// The picker's options (`--choose …` on the command line), read once.
fn choose_options() -> Option<&'static crate::choose::Choose> {
    static OPTS: std::sync::OnceLock<Option<crate::choose::Choose>> = std::sync::OnceLock::new();
    OPTS.get_or_init(|| {
        let args: Vec<String> = std::env::args().skip(1).collect();
        (args.first().map(String::as_str) == Some("--choose"))
            .then(|| crate::choose::parse(&args[1..]).ok())
            .flatten()
    })
    .as_ref()
}
