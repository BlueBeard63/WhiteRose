// Clocks' daemon-owned state, also used while the app is closed.
pragma Singleton
import QtQuick
import Quickshell
import "Clocks.js" as Format

Singleton {
    id: root
    readonly property var state: Daemon.clocks
    property real now: Date.now()
    readonly property var readout: Format.idle(state, now)
    onStateChanged: root.now = Date.now()
    Timer {
        running: root.readout !== null
        interval: 1000; repeat: true
        onTriggered: root.now = Date.now()
    }
    function open(view) {
        const id = "whiterose-clocks";
        if (Compositor.windows.some(w => w.app_id === id)) {
            Daemon.request({op:"clocks_open", args:{view:view}});
            Compositor.focusApp(id);
        } else {
            Compositor.launch(id, [Quickshell.env("WHITEROSE_CLOCKS") || id, "--view", view]);
        }
    }
}
