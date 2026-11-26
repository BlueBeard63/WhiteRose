//! WhiteRose Calculator: a Qt Quick app sharing the launcher's math engine.
extern crate whiterose_kit;

mod core;
#[cfg(test)]
#[path = "../../scratch.rs"]
mod scratch;
mod session;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

fn main() {
    let mut app = QGuiApplication::new();
    if let Some(mut a) = app.as_mut() {
        a.as_mut()
            .set_application_name(&QString::from("whiterose-calculator"));
        a.as_mut()
            .set_application_display_name(&QString::from("Calculator"));
    }
    let mut engine = QQmlApplicationEngine::new();
    if let Some(e) = engine.as_mut() {
        e.load(&QUrl::from("qrc:/qt/qml/WhiteRose/Calculator/qml/Main.qml"));
    }
    if let Some(a) = app.as_mut() {
        std::process::exit(a.exec());
    }
}
