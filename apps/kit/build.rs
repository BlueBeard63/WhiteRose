use cxx_qt_build::{CxxQtBuilder, QmlFile, QmlModule};

fn main() {
    let mut qml: Vec<QmlFile> = vec![QmlFile::from("qml/Theme.qml").singleton(true)];
    qml.extend(
        [
            "qml/Icon.qml",
            "qml/AppWindow.qml",
            "qml/Button.qml",
            "qml/IconButton.qml",
            "qml/Toggle.qml",
            "qml/Field.qml",
            "qml/Choice.qml",
            "qml/Segmented.qml",
            "qml/Badge.qml",
            "qml/Menu.qml",
            "qml/Highlight.qml",
        ]
        .map(QmlFile::from),
    );
    CxxQtBuilder::new_qml_module(QmlModule::new("WhiteRose.Kit").qml_files(qml))
        .qt_module("Quick")
        .files(["src/desktop.rs"])
        // Geist, the desktop's font, inside the binary.
        .qrc_resources([
            "fonts/Geist-Regular.ttf",
            "fonts/Geist-Medium.ttf",
            "fonts/Geist-SemiBold.ttf",
            "fonts/GeistMono-Regular.ttf",
            "fonts/GeistMono-Medium.ttf",
        ])
        .build()
        .export();
}
