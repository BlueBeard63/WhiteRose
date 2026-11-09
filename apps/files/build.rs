use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

fn main() {
    let mut qml: Vec<QmlFile> = vec![QmlFile::from("qml/Session.qml").singleton(true)];
    qml.extend(
        [
            "qml/Main.qml",
            "qml/Pane.qml",
            "qml/Sidebar.qml",
            "qml/AddressBar.qml",
            "qml/GridMode.qml",
            "qml/ListMode.qml",
            "qml/ColumnsMode.qml",
            "qml/Transfers.qml",
        ]
        .map(QmlFile::from),
    );
    CxxQtBuilder::new_qml_module(QmlModule::new("WhiteRose.Files").qml_files(qml))
        .qt_module("Quick")
        // Plain JS helpers ride along as resources (qmldir only takes
        // components). A named .qrc: unnamed resource lists from two crates both
        // become `resources_0_qrc` and one (the kit's fonts) gets dropped.
        .qrc("files.qrc")
        .files(["src/core.rs", "src/jobs.rs"])
        // Files on the clipboard (QML can only put text there).
        .cpp_file("cpp/clip.cpp")
        .qt_module("Gui")
        .build();
}
