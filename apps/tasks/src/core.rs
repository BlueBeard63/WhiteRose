//! Bounded asynchronous requests: socket I/O never runs on the Qt thread.
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
        #[qproperty(bool, connected)]
        #[qproperty(bool, busy)]
        type Tasks = super::TasksRust;
        #[qinvokable]
        fn refresh(self: Pin<&mut Tasks>);
        #[qinvokable]
        fn command(self: Pin<&mut Tasks>, op: &QString, args: &QString);
        #[qsignal]
        fn completed(self: Pin<&mut Tasks>);
    }
    impl cxx_qt::Threading for Tasks {}
    impl cxx_qt::Initialize for Tasks {}
}
use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use serde_json::{json, Value};
use std::{pin::Pin, time::Duration};
use whiterose_kit::daemon;

pub struct TasksRust {
    state: QString,
    error: QString,
    connected: bool,
    busy: bool,
    refreshing: bool,
}
impl Default for TasksRust {
    fn default() -> Self {
        Self {
            state: QString::from("{}"),
            error: QString::default(),
            connected: false,
            busy: false,
            refreshing: false,
        }
    }
}
impl cxx_qt::Initialize for qobject::Tasks {
    fn initialize(self: Pin<&mut Self>) {
        self.refresh();
    }
}
impl qobject::Tasks {
    pub fn refresh(mut self: Pin<&mut Self>) {
        if self.refreshing {
            return;
        }
        self.as_mut().rust_mut().refreshing = true;
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = daemon::request(json!({"op":"tasks_state"}), Duration::from_secs(3));
            let _ = thread.queue(move |mut obj| {
                obj.as_mut().rust_mut().refreshing = false;
                match result {
                    Ok(state) if state["system"].is_object() => {
                        obj.as_mut()
                            .set_state(QString::from(state.to_string().as_str()));
                        obj.as_mut().set_connected(true);
                    }
                    _ => obj.as_mut().set_connected(false),
                }
            });
        });
    }
    pub fn command(mut self: Pin<&mut Self>, op: &QString, args: &QString) {
        if self.busy || !self.connected {
            return;
        }
        let op = op.to_string();
        if !["tasks_end", "tasks_protect", "set"].contains(&op.as_str()) {
            return;
        }
        let Ok(args) = serde_json::from_str::<Value>(&args.to_string()) else {
            return;
        };
        let request = if op == "set" {
            // The only direct setting exposed by this app.
            json!({"op":"set","key":"memory.autoClose","value":args["on"]})
        } else {
            json!({"op":op,"args":args})
        };
        self.as_mut().set_busy(true);
        self.as_mut().set_error(QString::default());
        let thread = self.qt_thread();
        std::thread::spawn(move || {
            let result = daemon::request(request, Duration::from_secs(5));
            let _ = thread.queue(move |mut obj| {
                if let Err(error) = result {
                    obj.as_mut().set_error(QString::from(error.as_str()));
                }
                obj.as_mut().set_busy(false);
                obj.as_mut().completed();
                obj.as_mut().refresh();
            });
        });
    }
}
