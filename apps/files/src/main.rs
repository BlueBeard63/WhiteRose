//! WhiteRose Files: `whiterose-files [FOLDER...]`.

extern crate whiterose_kit;

mod archive;
mod core;
mod finder;
mod git;
mod hash;
mod jobs;
mod jump;
mod launch;
mod listing;
mod log;
mod looks;
mod ops;
mod places;
mod recent;
mod run;
mod search;
mod session;
mod tags;
mod trash;
mod views;
mod workspaces;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

fn main() {
    let mut app = QGuiApplication::new();
    if let Some(mut a) = app.as_mut() {
        a.as_mut()
            .set_application_name(&QString::from("whiterose-files"));
        a.as_mut()
            .set_application_display_name(&QString::from("Files"));
    }
    let mut engine = QQmlApplicationEngine::new();
    if let Some(e) = engine.as_mut() {
        e.load(&QUrl::from("qrc:/qt/qml/WhiteRose/Files/qml/Main.qml"));
    }
    if let Some(a) = app.as_mut() {
        std::process::exit(a.exec());
    }
}
