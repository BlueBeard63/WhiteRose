// The launcher's memory, in the settings database: how often each app was
// launched (launcher.launches, for ranking), recent commands
// (launcher.commands) and calculations (launcher.calculations). Also the
// programs on $PATH for the Commands chip, scanned once and refreshed at most
// once a minute when asked.
pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import "Search.js" as Search

Singleton {
    id: root
    readonly property var launches: Daemon.setting("launcher.launches", ({}))
    readonly property var history: Daemon.setting("launcher.commands", [])
    readonly property var calcHistory: Daemon.setting("launcher.calculations", [])
    property var commands: []

    function record(id) {
        const l = Object.assign({}, root.launches);
        l[id] = (l[id] ?? 0) + 1;
        Daemon.set("launcher.launches", l);
    }
    function recordCommand(cmd) { Daemon.set("launcher.commands", Search.pushHistory(root.history, cmd, 20)) }
    function recordCalculation(expr, result) {
        Daemon.set("launcher.calculations", Search.pushHistory(root.calcHistory, { expr, result }, 10, (a, b) => a.expr === b.expr));
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

}
