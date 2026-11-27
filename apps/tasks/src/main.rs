//! Native Task manager; sampling and process control belong to whiterosed.
extern crate whiterose_kit;
mod core;
use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

fn main() {
    let mut app = QGuiApplication::new();
    if let Some(mut a) = app.as_mut() {
        a.as_mut()
            .set_application_name(&QString::from("whiterose-tasks"));
        a.as_mut()
            .set_application_display_name(&QString::from("Task manager"));
    }
    let mut engine = QQmlApplicationEngine::new();
    if let Some(e) = engine.as_mut() {
        e.load(&QUrl::from("qrc:/qt/qml/WhiteRose/Tasks/qml/Main.qml"));
    }
    if let Some(a) = app.as_mut() {
        std::process::exit(a.exec());
    }
}
