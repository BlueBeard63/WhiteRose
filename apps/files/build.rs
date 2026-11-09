use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(QmlModule::new("WhiteRose.Files").qml_files([
        "qml/Main.qml",
        "qml/Pane.qml",
        "qml/Sidebar.qml",
        "qml/AddressBar.qml",
        "qml/GridMode.qml",
        "qml/ListMode.qml",
        "qml/ColumnsMode.qml",
    ]))
    .qt_module("Quick")
    // Plain JS helpers ride along as resources (qmldir only takes
    // components). A named .qrc: unnamed resource lists from two crates both
    // become `resources_0_qrc` and one (the kit's fonts) gets dropped.
    .qrc("files.qrc")
    .files(["src/core.rs"])
    .build();
}
