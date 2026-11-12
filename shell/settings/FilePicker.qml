// Choosing a file or a folder: WhiteRose Files as a picker
// (`whiterose-files --choose`), not a dialog over Settings. `open()` shows
// it; `picked(path)` says what was chosen (nothing if cancelled). Files is
// $WHITEROSE_FILES if set (tests, a checkout), else whiterose-files.
//   filters: [{ name, patterns: ["*.png", "image/*"] }], the first shown first
import QtQuick
import Quickshell
import Quickshell.Io

Item {
    id: root
    property string title: ""
    // Where it starts ("" for home).
    property string folder: ""
    property bool folders: false
    property var filters: []
    signal picked(string path)

    function open() {
        if (proc.running) return;
        const args = [Quickshell.env("WHITEROSE_FILES") || "whiterose-files", "--choose", "--json"];
        if (root.folders) args.push("--folder");
        if (root.title) args.push("--title", root.title);
        if (root.folder) args.push("--in", root.folder);
        for (const f of root.filters) args.push("--filter", f.name + ":" + f.patterns.join(" "));
        proc.command = args;
        proc.running = true;
    }

    Process {
        id: proc
        stdout: StdioCollector {
            onStreamFinished: {
                let paths = [];
                try { paths = JSON.parse(text) } catch (e) {}
                if (paths.length > 0) root.picked(paths[0]);
            }
        }
    }
}
