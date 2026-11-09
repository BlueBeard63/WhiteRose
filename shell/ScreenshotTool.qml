// The screenshot tool (SUPER+SHIFT+S; Figma: "Screenshot - Region"): pick a
// region, a window or the whole screen, an optional delay and whether to
// include the pointer, then copy it or save it. The compositor does the
// capture (after the tool has hidden itself, so it's never in the shot).
pragma Singleton
import QtQuick
import Quickshell
import "Shot.js" as Shot

Singleton {
    id: root

    property bool open: false
    property string mode: "region"      // region | window | screen
    property int delay: 0               // seconds
    property bool pointer: false
    // The selection (global logical {x, y, w, h}); kept for next time.
    property var region: null
    // Window mode: the window picked (from Compositor.windows).
    property var window: null
    readonly property bool canPointer: Compositor.outputCaps.pointer === true
    readonly property bool ready: mode === "screen" || (mode === "region" && region !== null && region.w >= 2 && region.h >= 2)
        || (mode === "window" && window !== null)

    function start() {
        if (ShellState.locked) return;
        ShellState.close();
        ShellState.closePanel();
        root.window = null;
        root.open = true;
        // Where each window is right now (for window mode).
        Compositor.send({ cmd: "windows_list" });
    }
    function cancel() { root.open = false }

    // Captures, then copies only (save false) or saves and copies.
    function capture(save) {
        if (!root.ready) return;
        let region = null;
        if (root.mode === "region") region = Shot.toRegion(root.region);
        else if (root.mode === "window") {
            region = root.window.geometry;
            Compositor.focusWindow(root.window.id); // in front of anything covering it
        }
        pending = { cmd: "capture", region: region, pointer: root.pointer && root.canPointer, save: save };
        root.open = false;
        // Long enough for the tool to be gone from the screen (and the
        // window raised), plus the chosen delay.
        shoot.interval = root.delay * 1000 + 250;
        shoot.restart();
    }
    property var pending: null
    Timer { id: shoot; onTriggered: { Compositor.send(root.pending); root.pending = null } }

    Connections {
        target: Compositor
        function onAction(name) { if (name === "screenshot_region") root.open ? root.cancel() : root.start() }
    }
    Connections {
        target: ShellState
        function onLockedChanged() { if (ShellState.locked) root.cancel() }
    }
}
