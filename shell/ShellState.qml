// Shell-wide UI state, also reachable from scripts and keybindings:
//   qs -p shell ipc call island set dock|clipboard|screenshots|launcher|idle
//   qs -p shell ipc call dock toggle|open|close|preview <app-id>
//   qs -p shell ipc call launcher toggle|open|close|search <text>
//   qs -p shell ipc call quick toggle|open|close
//   qs -p shell ipc call notifications toggle|open|close
//   qs -p shell ipc call session lock
pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

Singleton {
    id: root

    // Which view the centre island shows. The window switcher overrides this
    // while ALT+TAB is held (it is driven by the compositor).
    property string islandView: "idle"   // idle | dock | clipboard | screenshots | launcher
    readonly property var views: ["idle", "dock", "clipboard", "screenshots", "launcher"]

    function show(view) { if (views.includes(view)) { islandView = view; if (view !== "idle") panel = "" } }
    function toggle(view) { islandView === view ? close() : show(view) }
    function close() { islandView = "idle" }

    // The side panel hanging from the bar: "" (none) or "quick". Only one
    // thing is open at a time: opening it returns the island to idle.
    property string panel: ""
    function togglePanel(name) { if (panel === name) closePanel(); else { panel = name; islandView = "idle" } }
    function closePanel() { panel = "" }

    // The session lock screen (LockScreen.qml) shows while this is true; only
    // a correct password clears it.
    property bool locked: false
    function lock() { panel = ""; islandView = "idle"; locked = true }

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
