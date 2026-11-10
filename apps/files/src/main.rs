//! WhiteRose Files: `whiterose-files [FOLDER...]`, or `whiterose-files
//! --find …` to search from a terminal (cli.rs).

extern crate whiterose_kit;

mod apps;
mod archive;
mod bulk_rename;
mod cleanup;
mod cli;
mod cloudremotes;
mod cloudsync;
mod compare;
mod containers;
mod core;
mod diff;
mod finder;
mod git;
mod hash;
mod jobs;
mod journal;
mod json_view;
mod jump;
mod launch;
mod listing;
mod log;
mod looks;
mod markdown;
mod ops;
mod places;
mod prefs;
mod projects;
mod recent;
mod run;
mod search;
mod secure;
mod session;
mod shares;
mod shelf;
mod tags;
mod trash;
mod views;
mod workspaces;

use cxx_qt_lib::{QGuiApplication, QQmlApplicationEngine, QString, QUrl};

fn main() {
    // From a terminal: search, print, and done (no window).
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--find") => std::process::exit(cli::main(
            &args[1..],
            &mut std::io::stdout().lock(),
            &mut std::io::stderr(),
        )),
        Some("--help") | Some("-h") => {
            println!(
                "whiterose-files [FOLDER…]   open folders in Files\n{}",
                cli::HELP
            );
            return;
        }
        _ => {}
    }
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
