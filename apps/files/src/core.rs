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
        /// The git repository `path` is in ("" if none).
        #[qinvokable]
        #[cxx_name = "gitRoot"]
        fn git_root(self: &Fs, path: &QString) -> QString;
        /// Folders named on the command line, as JSON.
        #[qinvokable]
        fn arguments(self: &Fs) -> QString;
        /// The windows and tabs saved last time, cleaned up ({windows: []} if none).
        #[qinvokable]
        #[cxx_name = "loadSession"]
        fn load_session(self: &Fs) -> QString;
        #[qinvokable]
        #[cxx_name = "saveSession"]
        fn save_session(self: &Fs, json: &QString) -> bool;

        /// How `path` is shown: {mode, sortBy, descending, own, subfolders, from}.
        #[qinvokable]
        fn view(self: &Fs, path: &QString) -> QString;
        /// Remembers {mode, sortBy, descending} for this folder.
        #[qinvokable]
        #[cxx_name = "setView"]
        fn set_view(self: &Fs, path: &QString, json: &QString) -> bool;
        /// This folder lends its view to its subfolders (or stops).
        #[qinvokable]
        #[cxx_name = "setViewForSubfolders"]
        fn set_view_for_subfolders(self: &Fs, path: &QString, on: bool) -> bool;
        /// This view for every folder without one of its own.
        #[qinvokable]
        #[cxx_name = "setDefaultView"]
        fn set_default_view(self: &Fs, path: &QString, json: &QString) -> bool;
        #[qinvokable]
        #[cxx_name = "resetView"]
        fn reset_view(self: &Fs, path: &QString) -> bool;
        /// Hidden files shown (everywhere).
        #[qinvokable]
        #[cxx_name = "showsHidden"]
        fn shows_hidden(self: &Fs) -> bool;
        #[qinvokable]
        #[cxx_name = "setShowsHidden"]
        fn set_shows_hidden(self: &Fs, on: bool) -> bool;
        /// An image's size: "4032 × 3024" ("" if it can't be read).
        #[qinvokable]
        #[cxx_name = "imageSize"]
        fn image_dimensions(self: &Fs, path: &QString) -> QString;
        /// The details panel shown (everywhere).
        #[qinvokable]
        #[cxx_name = "showsDetails"]
        fn shows_details(self: &Fs) -> bool;
        #[qinvokable]
        #[cxx_name = "setShowsDetails"]
        fn set_shows_details(self: &Fs, on: bool) -> bool;
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

use crate::{launch, listing, looks, places, session, trash, views};

/// The Trash, as a place to go.
const TRASH: &str = "trash:";
/// Every drive, how full (the Storage page).
const DRIVES: &str = "drives:";

#[derive(Default)]
pub struct FsRust;

fn q(v: Value) -> QString {
    QString::from(v.to_string().as_str())
}

fn icon_path(p: Option<std::path::PathBuf>) -> String {
    p.map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_default()
}

impl qobject::Fs {
    pub fn list(
        &self,
        path: &QString,
        hidden: bool,
        sort: &QString,
        descending: bool,
        icon_size: i32,
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
                        link: m.file_type().is_symlink(),
                        dir,
                        path: p,
                        name,
                    })
                })
                .collect())
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
            _ => listing::Sort::Name,
        };
        listing::sort(&mut entries, by, descending);
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
                v
            })
            .collect();
        q(json!({"ok": true, "entries": out, "stamp": self.stamp_of(&path)}))
    }

    fn stamp_of(&self, path: &str) -> f64 {
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
        if path == DRIVES {
            return q(json!([{"name": "Storage", "path": DRIVES}]));
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

    pub fn resolve(&self, typed: &QString) -> QString {
        let t = typed.to_string();
        let t = t.trim().trim_start_matches("file://");
        if t.trim_end_matches('/') == TRASH {
            return QString::from(TRASH);
        }
        if t.trim_end_matches('/') == DRIVES {
            return QString::from(DRIVES);
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

    pub fn arguments(&self) -> QString {
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

    pub fn view(&self, path: &QString) -> QString {
        let found = views::load_from(&views::file()).find(&path.to_string());
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

    pub fn set_default_view(&self, path: &QString, json: &QString) -> bool {
        let Ok(v) = serde_json::from_str::<views::View>(&json.to_string()) else {
            return false;
        };
        views::change(|all| all.set_default(&path.to_string(), v))
    }

    pub fn reset_view(&self, path: &QString) -> bool {
        views::change(|all| all.reset(&path.to_string()))
    }

    pub fn image_dimensions(&self, path: &QString) -> QString {
        QString::from(qobject::image_size(&path.to_string()).as_str())
    }

    pub fn shows_details(&self) -> bool {
        views::load_from(&views::file()).details
    }

    pub fn set_shows_details(&self, on: bool) -> bool {
        views::change(|all| all.details = on)
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

    pub fn shows_hidden(&self) -> bool {
        views::load_from(&views::file()).hidden
    }

    pub fn set_shows_hidden(&self, on: bool) -> bool {
        views::change(|all| all.hidden = on)
    }

    pub fn up(&self, path: &QString) -> QString {
        let path = path.to_string();
        if path == TRASH || path == DRIVES || path == "/" {
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
