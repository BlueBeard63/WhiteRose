// Connection to whiterose-wm over $WHITEROSE_SOCKET (newline-delimited JSON).
// State arrives as events; nothing is polled.
pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland

Singleton {
    id: root

    readonly property bool available: socketPath !== ""
    readonly property string socketPath: Quickshell.env("WHITEROSE_SOCKET") ?? ""
    readonly property bool connected: socket.connected

    property int activeWorkspace: 1
    property int workspaceCount: 9
    property var occupiedWorkspaces: []
    // The active workspace's window layout on each screen, by screen name:
    // "floating", "dwindle" or "master".
    property var windowLayouts: ({})
    property string focusedAppId: ""
    property string focusedTitle: ""
    // Every window, most recently used first:
    // { id, app_id, title, workspace (1-based), focused }.
    property var windows: []
    // ALT+TAB / SUPER+ALT+TAB, run by the compositor: null when closed, else
    // { mode: "apps"|"windows", items: [{ app_id, windows: [id] }], selected, sub }.
    property var switcher: null
    // The snap layout picker while it's open (SnapPicker.qml draws it).
    property var snapPicker: null
    // Clipboard history, newest first:
    // { id, time, kind: "text"|"image"|"files", preview, image, files: [{ path, exists }], available }.
    property var clipboard: []
    // Newest screenshots in the Screenshots folder (absolute paths).
    property var screenshots: []
    // A window sits right under the idle island (e.g. a maximized title bar).
    // Screens (by name) whose idle island has a window right under it.
    property var islandCovered: []
    // Screens with a fullscreen window (the bar isn't drawn there).
    property var fullscreen: []

    // Connected monitors, built-in first: { id, strong, connector, make, model,
    // builtin, physical_mm, modes: [{ width, height, refresh (mHz), preferred }],
    // mode, scale, transform, position: [x, y], enabled, vrr, profile }.
    property var outputs: []
    // What the backend can change: { modes, rotation, position, disable, vrr }.
    property var outputCaps: ({})
    // While a layout is being tried: when it reverts (ms since the epoch), else 0.
    property real trialUntil: 0

    // Every shortcut: { id, name, group, keys: ["SUPER+Q"], defaults, command? }.
    property var shortcuts: []
    // Keyboard layouts in use ({ layout, variant, name }) and the active one.
    property var keyboardLayouts: []
    property int activeLayout: 0

    function windowById(id) { return root.windows.find(w => w.id === id) ?? null }

    // The QuickShell Toplevel (for live previews) behind a compositor window id.
    // Handles arrive in creation order, so the k-th window of an app (by id)
    // is the k-th toplevel with that app id.
    function toplevelFor(id) {
        const win = windowById(id);
        if (!win) return null;
        const appId = win.app_id ?? "";
        const k = root.windows.filter(w => (w.app_id ?? "") === appId).map(w => w.id).sort((a, b) => a - b).indexOf(id);
        return ToplevelManager.toplevels.values.filter(t => t.appId === appId)[k] ?? null;
    }

    // A shortcut the shell should handle, e.g. "screenshot_region", "app_launcher".
    signal action(string name)
    // A screenshot was saved (already on the clipboard as image/png).
    signal screenshotSaved(string path, int width, int height)
    signal screenshotFailed(string error)

    function workspace(n) { send({ cmd: "workspace", n: n }) }
    function moveToWorkspace(n) { send({ cmd: "move_to_workspace", n: n }) }
    function spawn(command) { send({ cmd: "spawn", command: command }) }
    function closeWindow() { send({ cmd: "close_window" }) }
    // The next window layout on `screenName` (as SUPER+T), saved for the workspace.
    function nextWindowLayout(screenName) { send({ cmd: "next_window_layout", output: screenName }) }
    function screenshot() { send({ cmd: "screenshot" }) }
    // Focus an app's most recent window, or cycle if it already has focus.
    function focusApp(appId) { send({ cmd: "focus_app", app_id: appId }) }
    function focusWindow(id) { send({ cmd: "focus_window", id: id }) }
    // Re-sends the history, re-checking copied files on disk.
    // Night Light: warm the screen to `kelvin` while on.
    function nightLight(on, kelvin) { send({ cmd: "night_light", on: on, temperature: kelvin }) }
    // Ends the session (Log Out).
    function quit() { send({ cmd: "quit" }) }
    // Shows each monitor's number and name on it (Displays page).
    function identifyOutputs() { send({ cmd: "identify_outputs" }) }
    // While Settings records a new shortcut, keys reach it instead of
    // triggering shortcuts (undone if Settings closes).
    function pauseShortcuts(on) { send({ cmd: "pause_shortcuts", on: on }) }
    function clipboardList() { send({ cmd: "clipboard_list" }) }
    function clipboardPaste(id) { send({ cmd: "clipboard_paste", id: id }) }
    // Makes the named copy of an image entry that a drag hands out.
    function clipboardPrepareDrag(id) { send({ cmd: "clipboard_prepare_drag", id: id }) }
    function clipboardDelete(id) { send({ cmd: "clipboard_delete", id: id }) }
    function clipboardClear() { send({ cmd: "clipboard_clear" }) }
    function screenshotList() { send({ cmd: "screenshot_list" }) }
    function screenshotCopy(path) { send({ cmd: "screenshot_copy", path: path }) }
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
            root.windowLayouts = msg.layouts ?? {};
            break;
        case "focus":
            root.focusedAppId = msg.app_id ?? "";
            root.focusedTitle = msg.title ?? "";
            break;
        case "windows":
            root.windows = msg.windows;
            break;
        case "switcher":
            root.switcher = msg.state;
            break;
        case "snap_picker":
            root.snapPicker = msg.picker;
            break;
        case "clipboard":
            root.clipboard = msg.entries;
            break;
        case "outputs":
            root.outputs = msg.outputs;
            root.outputCaps = msg.capabilities;
            root.trialUntil = msg.trial_until ?? 0;
            break;
        case "shortcuts":
            root.shortcuts = msg.shortcuts;
            break;
        case "keyboard":
            root.keyboardLayouts = msg.layouts;
            root.activeLayout = msg.active;
            break;
        case "island":
            root.islandCovered = Array.isArray(msg.covered) ? msg.covered : [];
            root.fullscreen = Array.isArray(msg.fullscreen) ? msg.fullscreen : [];
            break;
        case "lock":
            // A restarted shell finds the session still locked: lock again.
            if (msg.locked && !ShellState.locked) ShellState.locked = true;
            break;
        case "screenshots":
            root.screenshots = msg.files;
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
        onConnectedChanged: {
            if (connected) return;
            root.snapPicker = null; // (it closed with the compositor)
            if (root.available) reconnect.start();
        }
    }

    // If the compositor restarts, reconnect after a short pause (not a poll loop:
    // it only runs while disconnected).
    Timer {
        id: reconnect
        interval: 1000
        onTriggered: socket.connected = true
    }
}
