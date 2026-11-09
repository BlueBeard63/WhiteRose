// Connection to whiterose-wm over $WHITEROSE_SOCKET (newline-delimited JSON).
// State arrives as events; nothing is polled.
pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

Singleton {
    id: root

    readonly property bool available: socketPath !== ""
    readonly property string socketPath: Quickshell.env("WHITEROSE_SOCKET") ?? ""
    readonly property bool connected: socket.connected

    property int activeWorkspace: 1
    property int workspaceCount: 9
    property var occupiedWorkspaces: []
    property string focusedAppId: ""
    property string focusedTitle: ""

    // A shortcut the shell should handle, e.g. "screenshot_region", "app_launcher".
    signal action(string name)
    // A screenshot was saved (already on the clipboard as image/png).
    signal screenshotSaved(string path, int width, int height)
    signal screenshotFailed(string error)

    function workspace(n) { send({ cmd: "workspace", n: n }) }
    function moveToWorkspace(n) { send({ cmd: "move_to_workspace", n: n }) }
    function spawn(command) { send({ cmd: "spawn", command: command }) }
    function closeWindow() { send({ cmd: "close_window" }) }
    function screenshot() { send({ cmd: "screenshot" }) }
    // The compositor only deletes files it saved itself.
    function deleteScreenshot(path) { send({ cmd: "delete_screenshot", path: path }) }

    function send(obj) {
        if (!socket.connected) return;
        socket.write(JSON.stringify(obj) + "\n");
        socket.flush();
    }

    function handle(line) {
        let msg;
        try { msg = JSON.parse(line); } catch (e) { console.warn("whiterose ipc: bad message", line); return; }
        switch (msg.event) {
        case "workspaces":
            root.activeWorkspace = msg.active;
            root.workspaceCount = msg.count;
            root.occupiedWorkspaces = msg.occupied;
            break;
        case "focus":
            root.focusedAppId = msg.app_id ?? "";
            root.focusedTitle = msg.title ?? "";
            break;
        case "action":
            root.action(msg.name);
            break;
        case "screenshot":
            root.screenshotSaved(msg.path, msg.width, msg.height);
            break;
        case "screenshot_failed":
            root.screenshotFailed(msg.error);
            break;
        }
    }

    Socket {
        id: socket
        path: root.socketPath
        connected: root.available
        parser: SplitParser { onRead: line => root.handle(line) }
        onConnectedChanged: if (!connected && root.available) reconnect.start()
    }

    // If the compositor restarts, reconnect after a short pause (not a poll loop:
    // it only runs while disconnected).
    Timer {
        id: reconnect
        interval: 1000
        onTriggered: socket.connected = true
    }
}
