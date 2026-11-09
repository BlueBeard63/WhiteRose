use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("WhiteRose.Notepad").qml_files([
        "qml/Main.qml",
        "qml/Editor.qml",
        "qml/FindBar.qml",
        "qml/Highlight.qml",
    ]))
    .qt_module("Quick")
    .files(["src/core.rs"])
    .build();
}
