//! `Files` (QML singleton): Notepad's Rust side for QML. Reading and saving
//! (doc.rs), the session and scratch note (session.rs), and what a file is
//! (for highlighting and the status bar). Results come back as JSON.

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
        type Files = super::FilesRust;

        /// {ok, text, eol, mtime} or {ok: false, error}.
        #[qinvokable]
        fn open(self: &Files, path: &QString) -> QString;
        /// {ok, mtime} or {ok: false, error}. `eol` is "lf" or "crlf".
        #[qinvokable]
        fn save(self: &Files, path: &QString, text: &QString, eol: &QString) -> QString;
        /// When the file last changed (ms), 0 if it's gone.
        #[qinvokable]
        fn mtime(self: &Files, path: &QString) -> f64;
        #[qinvokable]
        fn session(self: &Files) -> QString;
        #[qinvokable]
        #[cxx_name = "saveSession"]
        fn save_session(self: &Files, json: &QString) -> bool;
        #[qinvokable]
        fn scratch(self: &Files) -> QString;
        #[qinvokable]
        #[cxx_name = "saveScratch"]
        fn save_scratch(self: &Files, text: &QString) -> bool;
        /// Files named on the command line (absolute paths), as JSON.
        #[qinvokable]
        fn arguments(self: &Files) -> QString;
        /// Canonical absolute path, shared by all ways of opening a file.
        #[qinvokable]
        fn normalize(self: &Files, path: &QString) -> QString;
        /// A later launch asks this window to open files (JSON array), or
        /// simply activate when the array is empty.
        #[qsignal]
        #[cxx_name = "openRequested"]
        fn open_requested(self: Pin<&mut Files>, paths: QString);
        /// {name, syntax} for a path: the highlighting definition's name.
        #[qinvokable]
        fn language(self: &Files, path: &QString) -> QString;
    }

    impl cxx_qt::Threading for Files {}
    impl cxx_qt::Initialize for Files {}
}

use cxx_qt_lib::QString;
use serde_json::json;
use std::path::Path;

use crate::{doc, lang, session};
use cxx_qt::{CxxQtType, Threading};
use std::pin::Pin;

#[derive(Default)]
pub struct FilesRust {
    arguments: Vec<String>,
}

impl cxx_qt::Initialize for qobject::Files {
    fn initialize(mut self: Pin<&mut Self>) {
        if let Some(primary) = crate::instance::take() {
            self.as_mut().rust_mut().arguments = primary.arguments.clone();
            let thread = self.qt_thread();
            primary.serve(move |paths| {
                thread
                    .queue(move |mut obj| {
                        obj.as_mut().open_requested(QString::from(
                            serde_json::to_string(&paths).unwrap().as_str(),
                        ));
                        // requestActivate() also supports other desktops. WhiteRose
                        // can bring the existing window back from another workspace.
                        if let Some(socket) = whiterose_kit::compositor::socket_path() {
                            whiterose_kit::compositor::send(
                                &socket,
                                r#"{"cmd":"focus_app","app_id":"whiterose-notepad"}"#,
                            );
                        }
                    })
                    .is_ok()
            });
        }
    }
}

fn q(v: serde_json::Value) -> QString {
    QString::from(v.to_string().as_str())
}

impl qobject::Files {
    pub fn open(&self, path: &QString) -> QString {
        match doc::read(Path::new(&path.to_string())) {
            Ok(d) => q(
                json!({"ok": true, "text": d.text, "eol": if d.eol == doc::Eol::Crlf { "crlf" } else { "lf" }, "mtime": d.mtime}),
            ),
            Err(e) => q(json!({"ok": false, "error": e})),
        }
    }

    pub fn save(&self, path: &QString, text: &QString, eol: &QString) -> QString {
        let eol = if eol.to_string() == "crlf" {
            doc::Eol::Crlf
        } else {
            doc::Eol::Lf
        };
        match doc::write(Path::new(&path.to_string()), &text.to_string(), &eol) {
            Ok(m) => q(json!({"ok": true, "mtime": m})),
            Err(e) => q(json!({"ok": false, "error": e})),
        }
    }

    pub fn mtime(&self, path: &QString) -> f64 {
        doc::mtime(Path::new(&path.to_string())) as f64
    }

    pub fn session(&self) -> QString {
        q(serde_json::to_value(session::load_from(&session::dir())).unwrap_or_default())
    }

    pub fn save_session(&self, json: &QString) -> bool {
        serde_json::from_str::<session::Session>(&json.to_string())
            .ok()
            .is_some_and(|s| session::save_to(&session::dir(), &s).is_ok())
    }

    pub fn scratch(&self) -> QString {
        QString::from(session::scratch_from(&session::dir()).as_str())
    }

    pub fn save_scratch(&self, text: &QString) -> bool {
        session::save_scratch_to(&session::dir(), &text.to_string()).is_ok()
    }

    pub fn arguments(&self) -> QString {
        q(json!(self.arguments))
    }

    pub fn normalize(&self, path: &QString) -> QString {
        let cwd = std::env::current_dir().unwrap_or_default();
        QString::from(
            crate::instance::normalize(Path::new(&path.to_string()), &cwd)
                .to_string_lossy()
                .as_ref(),
        )
    }

    pub fn language(&self, path: &QString) -> QString {
        let (name, syntax) = lang::detect(&path.to_string());
        q(json!({"name": name, "syntax": syntax}))
    }
}
