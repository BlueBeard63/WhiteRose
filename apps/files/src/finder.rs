//! `Search` (QML singleton): finding files for QML. `find` runs on its own
//! thread (a new search stops the one before) and `found` brings the hits:
//! as they come for a folder's tree, at once from the index for home and
//! the drives. The index is made again in the background, at the lowest
//! CPU and disk priority, when it's missing or over an hour old, or when
//! asked.

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
        /// The index is being made.
        #[qproperty(bool, indexing)]
        /// When it was made (unix ms; 0: never).
        #[qproperty(f64, indexed, cxx_name = "indexedAt")]
        type Search = super::SearchRust;

        /// `ask` (JSON): {text, scope: here | home | everywhere, here, kind,
        /// size, modified, hidden}. Returns its id.
        #[qinvokable]
        fn find(self: Pin<&mut Search>, ask: &QString) -> i32;
        #[qinvokable]
        fn stop(self: &Search);
        /// Makes the index again now.
        #[qinvokable]
        fn reindex(self: Pin<&mut Search>);
        /// Makes it if it's missing or old.
        #[qinvokable]
        #[cxx_name = "keepFresh"]
        fn keep_fresh(self: Pin<&mut Search>);

        /// {hits: [entry], seen, done}: entries like a listing's.
        #[qsignal]
        fn found(self: Pin<&mut Search>, id: i32, result: QString);
    }

    impl cxx_qt::Threading for Search {}
}

use std::{
    path::{Path, PathBuf},
    pin::Pin,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use serde_json::{json, Value};
use whiterose_kit::icons;

use crate::{places, search};

/// Results kept per search.
const KEEP: usize = 1000;
/// The index is made again when older than this (s).
const STALE: i64 = 3600;

#[derive(Default)]
pub struct SearchRust {
    indexing: bool,
    indexed: f64,
    next: i32,
    current: Option<Arc<AtomicBool>>,
    building: Option<Arc<AtomicBool>>,
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// What's indexed: home, and the drives in the sidebar other than the system's.
fn roots() -> Vec<PathBuf> {
    let home = places::home();
    let mut v = vec![home.clone()];
    for d in places::drives() {
        let p = PathBuf::from(&d.path);
        if p != Path::new("/") && !home.starts_with(&p) && !v.contains(&p) {
            v.push(p);
        }
    }
    v
}

/// Lowest CPU and disk priority for this thread.
pub fn background() {
    // SAFETY: plain syscalls on this thread's own id.
    unsafe {
        let tid = libc::syscall(libc::SYS_gettid) as libc::id_t;
        libc::setpriority(libc::PRIO_PROCESS, tid, 19);
        // IOPRIO_WHO_PROCESS = 1, IOPRIO_CLASS_IDLE = 3 (<< 13).
        libc::syscall(libc::SYS_ioprio_set, 1, tid, 3 << 13);
    }
}

fn entries(hits: &[search::Hit]) -> Vec<Value> {
    let ic = icons::shared();
    hits.iter()
        .map(|h| {
            let mime = if h.dir { "inode/directory".to_string() } else { ic.mime.type_of(&h.name).to_string() };
            let icon = if h.dir {
                ic.lookup("folder", 48, 1)
            } else {
                ic.mime.icon_names(&mime).iter().find_map(|n| ic.lookup(n, 48, 1))
            };
            json!({
                "name": h.name, "path": h.path, "dir": h.dir, "link": h.link, "size": h.size,
                "modified": h.modified, "hidden": h.name.starts_with('.'), "executable": false,
                "mime": mime, "icon": icon.map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
                "folder": Path::new(&h.path).parent().map(|p| p.to_string_lossy().into_owned()),
            })
        })
        .collect()
}

impl qobject::Search {
    pub fn find(mut self: Pin<&mut Self>, ask: &QString) -> i32 {
        self.stop();
        let ask: search::Ask = serde_json::from_str(&ask.to_string()).unwrap_or_default();
        self.as_mut().rust_mut().next += 1;
        let id = self.next;
        let stop = Arc::new(AtomicBool::new(false));
        self.as_mut().rust_mut().current = Some(stop.clone());
        let thread = self.qt_thread();
        let send = move |hits: &[search::Hit], seen: u64, done: bool| {
            let r = json!({"hits": entries(hits), "seen": seen, "done": done}).to_string();
            let _ = thread.queue(move |mut obj| obj.as_mut().found(id, QString::from(r.as_str())));
        };
        std::thread::spawn(move || {
            let q = search::parse(&ask.text);
            let f = search::Filters::from(&ask, now());
            let mime = |n: &str| icons::shared().mime.type_of(n).to_string();
            if ask.scope == "here" || ask.here.is_empty() && ask.scope.is_empty() {
                let mut send = send;
                let look = search::Look {
                    query: &q,
                    filters: &f,
                    hidden: ask.hidden,
                    mime: &mime,
                };
                search::walk(Path::new(&ask.here), &look, &stop, KEEP, &mut send);
                return;
            }
            let Some(ix) = search::Index::open(&search::index_file()) else {
                send(&[], 0, true);
                return;
            };
            let home = places::home();
            let want = |top: &Path| ask.scope == "everywhere" || top == home;
            let (hits, seen) = ix.find(&q, &f, &want, &mime, &stop, KEEP);
            if !stop.load(Ordering::Relaxed) {
                send(&hits, seen, true);
            }
        });
        id
    }

    pub fn stop(&self) {
        if let Some(s) = &self.current {
            s.store(true, Ordering::Relaxed);
        }
    }

    pub fn reindex(mut self: Pin<&mut Self>) {
        if self.indexing {
            return;
        }
        self.as_mut().set_indexing(true);
        let stop = Arc::new(AtomicBool::new(false));
        self.as_mut().rust_mut().building = Some(stop.clone());
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            background();
            let file = search::index_file();
            let _ = search::build(&roots(), &file, &stop);
            let at = search::index_age(&file) as f64 * 1000.0;
            let _ = thread.queue(move |mut obj| {
                obj.as_mut().set_indexed(at);
                obj.as_mut().set_indexing(false);
            });
        });
    }

    pub fn keep_fresh(mut self: Pin<&mut Self>) {
        let age = search::index_age(&search::index_file());
        self.as_mut().set_indexed(age as f64 * 1000.0);
        if age == 0 || now() - age > STALE {
            self.reindex();
        }
    }
}

impl Drop for SearchRust {
    fn drop(&mut self) {
        if let Some(s) = &self.building {
            s.store(true, Ordering::Relaxed);
        }
        if let Some(s) = &self.current {
            s.store(true, Ordering::Relaxed);
        }
    }
}
