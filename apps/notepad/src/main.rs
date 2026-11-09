//! WhiteRose Notepad: `whiterose-notepad [FILE...]`.

extern crate whiterose_kit;

mod core;
mod doc;
mod lang;
mod session;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

fn main() {
    let mut app = QGuiApplication::new();
    if let Some(mut a) = app.as_mut() {
        // Also the Wayland app id: the dock finds whiterose-notepad.desktop by it.
        a.as_mut()
            .set_application_name(&QString::from("whiterose-notepad"));
        a.as_mut()
            .set_application_display_name(&QString::from("Notepad"));
    }
    let mut engine = QQmlApplicationEngine::new();
    if let Some(e) = engine.as_mut() {
        e.load(&QUrl::from("qrc:/qt/qml/WhiteRose/Notepad/qml/Main.qml"));
    }
    if let Some(a) = app.as_mut() {
        std::process::exit(a.exec());
    }
}
