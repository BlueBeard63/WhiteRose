use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("WhiteRose.Clocks").qml_files([
        "qml/Main.qml",
        "qml/Readout.qml",
        "qml/TimeField.qml",
        "qml/Action.qml",
    ]))
    .qrc("clocks.qrc")
    .qt_module("Quick")
    .files(["src/core.rs"])
    .build();
}
