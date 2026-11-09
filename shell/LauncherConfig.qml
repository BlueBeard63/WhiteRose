// The launcher's memory, persisted to $XDG_CONFIG_HOME/whiterose/launcher.json:
// how often each app was launched (for ranking), recent commands and recent
// calculations. Also the programs on $PATH for the Commands chip, scanned
// once and refreshed at most once a minute when asked.
pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import "Search.js" as Search

Singleton {
    id: root
    readonly property var launches: adapter.launches
    readonly property var history: adapter.commands
    readonly property var calcHistory: adapter.calculations
    property var commands: []

    function record(id) {
        const l = Object.assign({}, adapter.launches);
        l[id] = (l[id] ?? 0) + 1;
        adapter.launches = l;
    }
    function recordCommand(cmd) { adapter.commands = Search.pushHistory(adapter.commands, cmd, 20) }
    function recordCalculation(expr, result) {
        adapter.calculations = Search.pushHistory(adapter.calculations, { expr, result }, 10, (a, b) => a.expr === b.expr);
    }

    property real scannedAt: 0
    function scanCommands() {
        if (scan.running || Date.now() - scannedAt < 60000) return;
        scannedAt = Date.now();
        scan.running = true;
    }
    Process {
        id: scan
        command: ["sh", "-c", "IFS=:; for d in $PATH; do [ -d \"$d\" ] && ls -1 \"$d\"; done 2>/dev/null | sort -u"]
        stdout: StdioCollector {
            onStreamFinished: root.commands = text.split("\n").filter(c => c !== "" && !c.startsWith("."))
        }
    }
    Component.onCompleted: scanCommands()

    FileView {
        path: DockConfig.configDir + "/launcher.json"
        watchChanges: true
        onFileChanged: reload()
        onAdapterUpdated: writeAdapter()
        onLoadFailed: error => { if (error === FileViewError.FileNotFound) writeAdapter() }
        JsonAdapter {
            id: adapter
            property var launches: ({})
            property var commands: []
            property var calculations: []
        }
    }
}
