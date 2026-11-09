// Shell-wide UI state, also reachable from scripts and keybindings:
//   qs -p shell ipc call dock toggle | open | close
pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

Singleton {
    id: root
    property bool dockOpen: false

    IpcHandler {
        target: "dock"
        function toggle(): void { root.dockOpen = !root.dockOpen }
        function open(): void { root.dockOpen = true }
        function close(): void { root.dockOpen = false }
    }
}
