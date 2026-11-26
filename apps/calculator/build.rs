use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("WhiteRose.Calculator").qml_files(["qml/Main.qml", "qml/CalcKey.qml"]),
    )
    .qrc("calculator.qrc")
    .qt_module("Quick")
    .qt_module("Gui")
    .files(["src/core.rs"])
    .cpp_file("cpp/clipboard.cpp")
    .build();
}
