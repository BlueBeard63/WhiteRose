use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("WhiteRose.Tasks").qml_files([
        "qml/Main.qml",
        "qml/Action.qml",
        "qml/Graph.qml",
    ]))
    .qrc("tasks.qrc")
    .qt_module("Quick")
    .files(["src/core.rs"])
    .build();
}
