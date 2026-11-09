// Shell-wide UI state, also reachable from scripts and keybindings:
//   qs -p shell ipc call dock toggle | open | close
pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

Singleton {
    id: root
    property bool dockOpen: false
    // Desktop entry id whose windows the dock should preview (set by hover, or
    // `qs ipc call dock preview <id>` for keyboard use and scripting).
    property string dockPreviewKey: ""

    IpcHandler {
        target: "dock"
        function toggle(): void { root.dockOpen = !root.dockOpen }
        function open(): void { root.dockOpen = true }
        function close(): void { root.dockOpen = false }
        function preview(id: string): void { root.dockOpen = true; root.dockPreviewKey = id }
    }
}
