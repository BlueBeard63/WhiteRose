use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("WhiteRose.Notepad").qml_files([
        "qml/Main.qml",
        "qml/Editor.qml",
        "qml/FindBar.qml",
    ]))
    // The JS the QML imports (a named .qrc: see apps/files/build.rs).
    .qrc("notepad.qrc")
    .qt_module("Quick")
    .files(["src/core.rs"])
    .build();
}
