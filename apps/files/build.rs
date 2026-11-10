use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

fn main() {
    // (files.qrc takes it from the Settings module; a change rebuilds.)
    println!("cargo::rerun-if-changed=../../shell/settings/modules/files/FilesKeys.js");
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
            "qml/Properties.qml",
            "qml/DragSource.qml",
            "qml/FolderDrop.qml",
            "qml/TabView.qml",
            "qml/Preview.qml",
            "qml/DetailsPanel.qml",
            "qml/QuickLook.qml",
            "qml/DrivesMode.qml",
            "qml/KeysDialog.qml",
            "qml/FolderIcon.qml",
            "qml/LookDialog.qml",
            "qml/GalleryMode.qml",
            "qml/CardsMode.qml",
            "qml/UsageMode.qml",
            "qml/GridCell.qml",
            "qml/CardCell.qml",
            "qml/GroupedGrid.qml",
            "qml/GroupHeading.qml",
            "qml/TagDots.qml",
            "qml/TagDialog.qml",
        ]
        .map(QmlFile::from),
    );
    CxxQtBuilder::new_qml_module(QmlModule::new("WhiteRose.Files").qml_files(qml))
        .qt_module("Quick")
        // Plain JS helpers ride along as resources (qmldir only takes
        // components). A named .qrc: unnamed resource lists from two crates both
        // become `resources_0_qrc` and one (the kit's fonts) gets dropped.
        .qrc("files.qrc")
        .files(["src/core.rs", "src/jobs.rs", "src/finder.rs"])
        // Files on the clipboard (QML can only put text there).
        .cpp_file("cpp/clip.cpp")
        .cpp_file("cpp/image.cpp")
        .qt_module("Gui")
        .build();
}
