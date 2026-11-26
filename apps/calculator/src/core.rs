//! Calculator's persistent history/preferences and the Qt clipboard.

#[cxx_qt::bridge]
pub mod qobject {
    unsafe extern "C++" {
        include!("cxx-qt-lib/qstring.h");
        type QString = cxx_qt_lib::QString;
        include!("whiterose-calculator/cpp/clipboard.h");
        #[cxx_name = "calculatorCopy"]
        fn calculator_copy(text: &str);
    }
    extern "RustQt" {
        #[qobject]
        #[qml_element]
        #[qml_singleton]
        type Store = super::StoreRust;

        #[qinvokable]
        fn load(self: &Store) -> QString;
        #[qinvokable]
        fn record(self: &Store, expression: &QString, result: &QString, angle: &QString)
            -> QString;
        #[qinvokable]
        fn preferences(self: &Store, scientific: bool, angle: &QString, history: bool) -> QString;
        #[qinvokable]
        fn clear(self: &Store) -> QString;
        #[qinvokable]
        fn copy(self: &Store, text: &QString);
    }
}

use crate::session::{self, Angle};
use cxx_qt_lib::QString;
use serde_json::json;

#[derive(Default)]
pub struct StoreRust;

fn angle(text: &QString) -> Angle {
    if text.to_string() == "rad" {
        Angle::Rad
    } else {
        Angle::Deg
    }
}

fn response(result: Result<session::Session, String>) -> QString {
    let value = match result {
        Ok(state) => json!({"ok": true, "state": state}),
        Err(error) => json!({"ok": false, "error": error}),
    };
    QString::from(value.to_string().as_str())
}

impl qobject::Store {
    pub fn load(&self) -> QString {
        response(Ok(session::load(&session::dir())))
    }
    pub fn record(&self, expression: &QString, result: &QString, unit: &QString) -> QString {
        response(session::record(
            &session::dir(),
            session::Entry {
                expression: expression.to_string(),
                result: result.to_string(),
                angle: angle(unit),
            },
        ))
    }
    pub fn preferences(&self, scientific: bool, unit: &QString, history: bool) -> QString {
        response(session::change(&session::dir(), |state| {
            state.scientific = scientific;
            state.angle = angle(unit);
            state.show_history = history;
        }))
    }
    pub fn clear(&self) -> QString {
        response(session::change(&session::dir(), |state| {
            state.history.clear()
        }))
    }
    pub fn copy(&self, text: &QString) {
        qobject::calculator_copy(&text.to_string());
    }
}
