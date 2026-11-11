// Shell-wide UI state, also reachable from scripts and keybindings:
//   qs -p shell ipc call island set dock|clipboard|screenshots|launcher|idle
//   qs -p shell ipc call dock toggle|open|close|preview <app-id>
//   qs -p shell ipc call launcher toggle|open|close|search <text>
//   qs -p shell ipc call quick toggle|open|close
//   qs -p shell ipc call notifications toggle|open|close
//   qs -p shell ipc call calendar toggle|open|close
//   qs -p shell ipc call session lock
pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

Singleton {
    id: root

    // Which view the centre island shows. The window switcher overrides this
    // while ALT+TAB is held (it is driven by the compositor).
    property string islandView: "idle"
    // The island's views: one file each in shell/island/ (the name is the
    // file's; see IslandView.qml), found when the shell starts.
    property var islandViews: []
    readonly property var views: ["idle"].concat(islandViews)
    Process {
        running: true
        command: ["sh", "-c", "for f in \"$1\"/*.qml; do [ -f \"$f\" ] && basename \"$f\" .qml; done; true", "sh", Quickshell.shellDir + "/island"]
        stdout: StdioCollector { onStreamFinished: root.islandViews = text.split("\n").filter(n => /^[a-z][a-z0-9-]*$/.test(n)) }
    }

    function show(view) { if (views.includes(view)) { islandView = view; if (view !== "idle") panel = "" } }
    function toggle(view) { islandView === view ? close() : show(view) }
    function close() { islandView = "idle" }

    // The side panel hanging from the bar: "" (none) or "quick". Only one
    // thing is open at a time: opening it returns the island to idle.
    property string panel: ""
    function togglePanel(name) { if (panel === name) closePanel(); else { panel = name; islandView = "idle" } }
    function closePanel() { panel = "" }

    // Opens the Settings app (its own window), at `page` if given.
    function openSettings(page) {
        panel = ""; islandView = "idle";
        Quickshell.execDetached([Quickshell.shellDir + "/bin/whiterose-settings", page ?? ""]);
    }

    // The session lock screen (LockScreen.qml) shows while this is true; only
    // a correct password clears it.
    property bool locked: false
    function lock() { panel = ""; islandView = "idle"; locked = true }

    // A new WhiteRose version didn't start (the session launcher went back):
    // { version, reason, attempts, tries, log, back_on }, from whiterosed.
    // The island shows the "couldn't start" prompt once per session; Try
    // again / Stay settle it (Settings › Updates shows it too).
    property var updateFailure: null
    property bool failureShown: false
    function noteUpdates(st) {
        root.updateFailure = st?.failure ?? null;
        if (root.updateFailure && !root.failureShown && !root.locked) {
            root.failureShown = true;
            root.show("recovery");
        } else if (!root.updateFailure && root.islandView === "recovery") {
            root.close();
        }
    }
    Connections {
        target: Daemon
        function onUpdatesEvent(st) { root.noteUpdates(st) }
        function onConnectedChanged() { root.askUpdates() }
    }
    function askUpdates() {
        if (Daemon.connected) Daemon.request({ op: "updates" }, v => { if (v) root.noteUpdates(v) });
    }
    Component.onCompleted: askUpdates()

    // An item is being dragged out of the island: keep it open meanwhile.
    property bool dragging: false

    // Desktop entry id whose windows the dock should preview (hover, or IPC).
    property string dockPreviewKey: ""
    // Text to pre-fill when the launcher opens (from `launcher search <text>`).
    property string launcherQuery: ""

    IpcHandler {
        target: "island"
        function set(view: string): void { root.show(view) }
    }
    IpcHandler {
        target: "dock"
        function toggle(): void { root.toggle("dock") }
        function open(): void { root.show("dock") }
        function close(): void { root.close() }
        function preview(id: string): void { root.show("dock"); root.dockPreviewKey = id }
    }
    IpcHandler {
        target: "quick"
        function toggle(): void { root.togglePanel("quick") }
        function open(): void { if (root.panel !== "quick") root.togglePanel("quick") }
        function close(): void { root.closePanel() }
    }
    IpcHandler {
        target: "notifications"
        function toggle(): void { root.togglePanel("notifications") }
        function open(): void { if (root.panel !== "notifications") root.togglePanel("notifications") }
        function close(): void { root.closePanel() }
    }
    IpcHandler {
        target: "calendar"
        function toggle(): void { root.togglePanel("calendar") }
        function open(): void { if (root.panel !== "calendar") root.togglePanel("calendar") }
        function close(): void { root.closePanel() }
    }
    IpcHandler {
        target: "session"
        function lock(): void { root.lock() }
    }
    IpcHandler {
        target: "launcher"
        function toggle(): void { root.toggle("launcher") }
        function open(): void { root.show("launcher") }
        function close(): void { root.close() }
        function search(text: string): void {
            root.launcherQuery = text;
            if (root.islandView === "launcher") root.launcherQueryChanged(); else root.show("launcher");
        }
    }
}
