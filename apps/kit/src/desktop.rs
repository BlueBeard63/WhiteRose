//! `Desktop` (QML singleton, `import WhiteRose.Kit`): the session's settings
//! for WhiteRose's apps. The appearance ones as properties (Theme.qml turns
//! them into colours), any setting as JSON, and changes as they happen.

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
        /// dark | light | auto
        #[qproperty(QString, theme)]
        #[qproperty(QString, accent)]
        #[qproperty(QString, font)]
        /// compact | default | large
        #[qproperty(QString, size)]
        /// The daemon is connected (settings are real, not defaults).
        #[qproperty(bool, connected)]
        type Desktop = super::DesktopRust;

        /// A setting as JSON ("null" if unknown).
        #[qinvokable]
        fn setting(self: &Desktop, key: &QString) -> QString;
        /// Changes a setting (JSON value); false if the daemon isn't there.
        #[qinvokable]
        fn set(self: &Desktop, key: &QString, json: &QString) -> bool;
        /// Daytime for the Auto theme, at the weather location if there is
        /// one; `local_hour` is used without one (07-19).
        #[qinvokable]
        fn daytime(self: &Desktop, local_hour: i32) -> bool;

        #[qsignal]
        #[cxx_name = "settingChanged"]
        fn setting_changed(self: Pin<&mut Desktop>, key: QString);
    }

    impl cxx_qt::Threading for Desktop {}
    impl cxx_qt::Initialize for Desktop {}
}

use std::{
    collections::HashMap,
    pin::Pin,
    sync::{Arc, Mutex},
};

use cxx_qt::{CxxQtType, Threading};
use cxx_qt_lib::QString;
use serde_json::Value;

use crate::daemon::{self, Change, Daemon};

pub struct DesktopRust {
    theme: QString,
    accent: QString,
    font: QString,
    size: QString,
    connected: bool,
    values: Arc<Mutex<HashMap<String, Value>>>,
    daemon: Option<Daemon>,
}

impl Default for DesktopRust {
    fn default() -> Self {
        DesktopRust {
            theme: QString::from("dark"),
            accent: QString::from("#E8A3AE"),
            font: QString::from("Geist"),
            size: QString::from("default"),
            connected: false,
            values: Default::default(),
            daemon: None,
        }
    }
}

impl cxx_qt::Initialize for qobject::Desktop {
    fn initialize(mut self: Pin<&mut Self>) {
        let thread = self.qt_thread();
        let values = self.values.clone();
        let d = Daemon::start(daemon::socket_path(), move |change| {
            // On the daemon thread: remember it, then update QML on the main thread.
            let keys: Vec<String> = {
                let mut v = values.lock().unwrap();
                match change {
                    Change::All(all) => {
                        *v = all;
                        v.keys().cloned().collect()
                    }
                    Change::One(k, val) => {
                        v.insert(k.clone(), val);
                        vec![k]
                    }
                }
            };
            let _ = thread.queue(move |mut obj| obj.as_mut().apply(&keys));
        });
        self.as_mut().rust_mut().daemon = Some(d);
    }
}

impl qobject::Desktop {
    fn value(&self, key: &str) -> Option<Value> {
        self.values.lock().unwrap().get(key).cloned()
    }

    fn apply(mut self: Pin<&mut Self>, keys: &[String]) {
        self.as_mut().set_connected(true);
        let text = |s: &Self, k: &str, default: &str| {
            QString::from(
                s.value(k)
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_else(|| default.into())
                    .as_str(),
            )
        };
        let theme = text(&self, "appearance.theme", "dark");
        let accent = text(&self, "appearance.accent", "#E8A3AE");
        let font = text(&self, "appearance.font", "Geist");
        let size = text(&self, "appearance.size", "default");
        self.as_mut().set_theme(theme);
        self.as_mut().set_accent(accent);
        self.as_mut().set_font(font);
        self.as_mut().set_size(size);
        for k in keys {
            self.as_mut().setting_changed(QString::from(k.as_str()));
        }
    }

    pub fn setting(&self, key: &QString) -> QString {
        let v = self.value(&key.to_string()).unwrap_or(Value::Null);
        QString::from(v.to_string().as_str())
    }

    pub fn set(&self, key: &QString, json: &QString) -> bool {
        let Ok(v) = serde_json::from_str::<Value>(&json.to_string()) else {
            return false;
        };
        self.daemon
            .as_ref()
            .is_some_and(|d| d.set(&key.to_string(), v))
    }

    pub fn daytime(&self, local_hour: i32) -> bool {
        let place = match (
            self.value("weather.latitude"),
            self.value("weather.longitude"),
        ) {
            (Some(Value::Number(a)), Some(Value::Number(b))) => a.as_f64().zip(b.as_f64()),
            _ => None,
        };
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as f64)
            .unwrap_or(0.0);
        crate::sun::is_day(now, place, local_hour.max(0) as u32)
    }
}
