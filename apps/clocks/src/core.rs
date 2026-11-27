//! Async daemon requests and live snapshots. Never block Qt on socket I/O.
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
        #[qproperty(QString, state)]
        #[qproperty(QString, error)]
        #[qproperty(QString, results)]
        #[qproperty(bool, connected)]
        #[qproperty(bool, busy)]
        #[qproperty(bool, searching)]
        #[qproperty(QString, view)]
        #[qproperty(bool, previewing)]
        type Clock = super::ClockRust;

        #[qinvokable]
        fn command(self: Pin<&mut Clock>, action: &QString, args: &QString);
        #[qinvokable]
        fn refresh(self: Pin<&mut Clock>);
        #[qinvokable]
        fn search(self: Pin<&mut Clock>, query: &QString);
        #[qinvokable]
        fn preview(self: Pin<&mut Clock>, options: &QString);
        #[qinvokable]
        #[cxx_name = "stopPreview"]
        fn stop_preview(self: Pin<&mut Clock>);
        #[qinvokable]
        #[cxx_name = "importSound"]
        fn import_sound(self: Pin<&mut Clock>, path: &QString);
        #[qsignal]
        #[cxx_name = "soundImported"]
        fn sound_imported(self: Pin<&mut Clock>, sound: QString);
        #[qsignal]
        #[cxx_name = "previewStopped"]
        fn preview_stopped(self: Pin<&mut Clock>);
        #[qinvokable]
        #[cxx_name = "bootTime"]
        fn boot_time(self: &Clock) -> f64;
    }
    impl cxx_qt::Threading for Clock {}
    impl cxx_qt::Initialize for Clock {}
}
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use serde_json::{json, Value};
use std::{
    io::{BufRead, BufReader, Write},
    os::unix::net::UnixStream,
    pin::Pin,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use whiterose_kit::daemon;

pub struct ClockRust {
    state: QString,
    error: QString,
    results: QString,
    connected: bool,
    busy: bool,
    searching: bool,
    view: QString,
    previewing: bool,
    preview_seq: u64,
    preview_client: String,
    revision: u64,
    search_id: Arc<AtomicU64>,
}
impl Default for ClockRust {
    fn default() -> Self {
        let args: Vec<_> = std::env::args().collect();
        let view = args
            .windows(2)
            .find(|p| p[0] == "--view")
            .map(|p| p[1].as_str())
            .unwrap_or("timers");
        Self {
            state: QString::from("{}"),
            error: QString::default(),
            results: QString::from("[]"),
            connected: false,
            busy: false,
            searching: false,
            view: QString::from(view),
            previewing: false,
            preview_seq: 0,
            preview_client: format!("{}-{:?}", std::process::id(), std::time::SystemTime::now()),
            revision: 0,
            search_id: Arc::new(AtomicU64::new(0)),
        }
    }
}
impl cxx_qt::Initialize for qobject::Clock {
    fn initialize(self: Pin<&mut Self>) {
        let thread = self.qt_thread();
        std::thread::spawn(move || loop {
            if let Ok(mut stream) = UnixStream::connect(daemon::socket_path()) {
                let _ = thread.queue(|mut obj| obj.as_mut().rust_mut().revision = 0);
                let _ = stream.write_all(b"{\"id\":1,\"op\":\"clocks_state\"}\n");
                for line in BufReader::new(stream).lines() {
                    let Ok(line) = line else { break };
                    let Ok(v) = serde_json::from_str::<Value>(&line) else {
                        continue;
                    };
                    if v["event"] == "clocks" || v["reply"] == 1 {
                        let state = if v["event"] == "clocks" {
                            v.get("state")
                        } else {
                            v.get("value")
                        };
                        if let Some(state) = state.cloned() {
                            if thread
                                .queue(move |mut obj| obj.as_mut().receive(state))
                                .is_err()
                            {
                                return;
                            }
                        } else if let Some(e) = v["error"].as_str() {
                            let error = QString::from(e);
                            let _ = thread.queue(move |mut obj| obj.as_mut().set_error(error));
                        }
                    } else if v["event"] == "clocks_open" {
                        if let Some(view) = v["view"].as_str() {
                            let view = QString::from(view);
                            let _ = thread.queue(move |mut obj| obj.as_mut().set_view(view));
                        }
                    }
                }
            }
            if thread
                .queue(|mut obj| obj.as_mut().set_connected(false))
                .is_err()
            {
                return;
            }
            std::thread::sleep(Duration::from_secs(1));
        });
    }
}
impl qobject::Clock {
    pub fn preview(mut self: Pin<&mut Self>, options: &QString) {
        let Ok(options) = serde_json::from_str::<Value>(&options.to_string()) else {
            return;
        };
        self.as_mut().rust_mut().preview_seq += 1;
        let sequence = self.preview_seq;
        let client = self.preview_client.clone();
        self.as_mut().set_previewing(true);
        self.as_mut().set_error(QString::default());
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = daemon::request(
                json!({"op":"clocks_preview","args":{"client":client,"sequence":sequence,"options":options}}),
                Duration::from_secs(10),
            );
            let _ = thread.queue(move |mut obj| {
                if obj.preview_seq != sequence {
                    return;
                }
                obj.as_mut().set_previewing(false);
                if let Err(error) = result {
                    obj.as_mut().set_error(QString::from(error.as_str()))
                }
            });
        });
    }
    pub fn stop_preview(mut self: Pin<&mut Self>) {
        if !self.previewing {
            return;
        }
        self.as_mut().rust_mut().preview_seq += 1;
        let sequence = self.preview_seq;
        let client = self.preview_client.clone();
        self.as_mut().set_previewing(false);
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let _ = daemon::request(
                json!({"op":"clocks_preview","args":{"client":client,"sequence":sequence,"stop":true}}),
                Duration::from_secs(2),
            );
            let _ = thread.queue(|mut obj| obj.as_mut().preview_stopped());
        });
    }
    pub fn import_sound(mut self: Pin<&mut Self>, path: &QString) {
        if self.busy {
            return;
        }
        let path = path.to_string();
        self.as_mut().set_busy(true);
        self.as_mut().set_error(QString::default());
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = daemon::request(
                json!({"op":"clocks_import_sound","args":{"path":path}}),
                Duration::from_secs(20),
            );
            let _ = thread.queue(move |mut obj| {
                match result {
                    Ok(result) => {
                        if let Some(state) = result.get("state") {
                            obj.as_mut().receive(state.clone())
                        }
                        if let Some(sound) = result["sound"].as_str() {
                            obj.as_mut().sound_imported(QString::from(sound))
                        }
                    }
                    Err(error) => obj.as_mut().set_error(QString::from(error.as_str())),
                }
                obj.as_mut().set_busy(false);
            });
        });
    }
    fn receive(mut self: Pin<&mut Self>, state: Value) {
        let revision = state["revision"].as_u64().unwrap_or(0);
        if revision < self.revision {
            return;
        }
        self.as_mut().rust_mut().revision = revision;
        self.as_mut()
            .set_state(QString::from(state.to_string().as_str()));
        self.as_mut().set_connected(true);
    }
    pub fn command(mut self: Pin<&mut Self>, action: &QString, args: &QString) {
        if self.busy {
            return;
        }
        let Ok(mut args) = serde_json::from_str::<Value>(&args.to_string()) else {
            return;
        };
        if !args.is_object() {
            return;
        }
        args["action"] = json!(action.to_string());
        self.as_mut().set_busy(true);
        self.as_mut().set_error(QString::default());
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = daemon::request(
                json!({"op":"clocks_command","args":args}),
                Duration::from_secs(10),
            );
            let _ = thread.queue(move |mut obj| {
                match result {
                    Ok(state) => obj.as_mut().receive(state),
                    Err(error) => obj.as_mut().set_error(QString::from(error.as_str())),
                }
                obj.as_mut().set_busy(false);
            });
        });
    }
    pub fn refresh(self: Pin<&mut Self>) {
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = daemon::request(json!({"op":"clocks_state"}), Duration::from_secs(5));
            let _ = thread.queue(move |mut obj| match result {
                Ok(state) => obj.as_mut().receive(state),
                Err(error) => obj.as_mut().set_error(QString::from(error.as_str())),
            });
        });
    }
    pub fn search(mut self: Pin<&mut Self>, query: &QString) {
        let query = query.to_string();
        let generation = self.search_id.fetch_add(1, Ordering::SeqCst) + 1;
        let current = self.search_id.clone();
        self.as_mut().set_results(QString::from("[]"));
        self.as_mut().set_searching(query.trim().len() >= 2);
        self.as_mut().set_error(QString::default());
        if query.trim().len() < 2 {
            return;
        }
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = daemon::request(
                json!({"op":"clocks_cities","args":{"query":query}}),
                Duration::from_secs(12),
            );
            let _ = thread.queue(move |mut obj| {
                if current.load(Ordering::SeqCst) != generation {
                    return;
                }
                obj.as_mut().set_searching(false);
                match result {
                    Ok(results) => obj
                        .as_mut()
                        .set_results(QString::from(results.to_string().as_str())),
                    Err(error) => obj.as_mut().set_error(QString::from(error.as_str())),
                }
            });
        });
    }
    pub fn boot_time(&self) -> f64 {
        let mut ts: libc::timespec = unsafe { std::mem::zeroed() };
        // SAFETY: ts is an initialized, writable timespec.
        if unsafe { libc::clock_gettime(libc::CLOCK_BOOTTIME, &mut ts) } == 0 {
            ts.tv_sec as f64 * 1000.0 + ts.tv_nsec as f64 / 1e6
        } else {
            0.0
        }
    }
}
