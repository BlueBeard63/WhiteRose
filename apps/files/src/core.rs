//! `Fs` (QML singleton): the file manager's Rust side for QML. Results come
//! back as JSON.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
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
        /// An icon-theme name's file ("" if none).
        #[qinvokable]
        fn icon(self: &Fs, name: &QString, size: i32) -> QString;
        #[qinvokable]
        fn home(self: &Fs) -> QString;
        /// A typed location made absolute (~ expanded), "" if it isn't a folder.
        #[qinvokable]
        fn resolve(self: &Fs, typed: &QString) -> QString;
        /// Folders named on the command line, as JSON.
        #[qinvokable]
        fn arguments(self: &Fs) -> QString;
    }
}

use std::{os::unix::fs::MetadataExt, path::Path};

use cxx_qt_lib::QString;
use serde_json::{json, Value};
use whiterose_kit::icons;

use crate::{listing, places};

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
        let mut entries =
            match listing::read(Path::new(&path), hidden, |n| ic.mime.type_of(n).to_string()) {
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
                v
            })
            .collect();
        q(json!({"ok": true, "entries": out, "stamp": self.stamp_of(&path)}))
    }

    fn stamp_of(&self, path: &str) -> f64 {
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
        let with_icon = |p: places::Place| {
            let file = icon_path(
                ic.lookup(&p.icon, 20, 1)
                    .or_else(|| ic.lookup("folder", 20, 1)),
            );
            json!({"name": p.name, "path": p.path, "icon": file})
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
            "folders": places::folders().into_iter().map(with_icon).collect::<Vec<_>>(),
            "bookmarks": places::bookmarks().into_iter().map(with_icon).collect::<Vec<_>>(),
            "drives": drives,
        }))
    }

    pub fn set_bookmark(&self, path: &QString, on: bool) -> bool {
        places::set_bookmark(&path.to_string(), on).is_ok()
    }

    pub fn crumbs(&self, path: &QString) -> QString {
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

    pub fn home(&self) -> QString {
        QString::from(places::home().to_string_lossy().as_ref())
    }

    pub fn resolve(&self, typed: &QString) -> QString {
        let t = typed.to_string();
        let t = t.trim().trim_start_matches("file://");
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
}
