// Quick settings toggles owned by the shell, kept across restarts in
// $XDG_STATE_HOME/whiterose/quick.json:
// - Do Not Disturb: the notification server holds popups while it's on.
// - Night Light: the compositor warms the screen; it switches itself off at
//   the next 07:00.
pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import "QuickSettings.js" as QS

Singleton {
    id: root

    readonly property bool doNotDisturb: adapter.doNotDisturb
    readonly property bool nightLight: adapter.nightLightUntil > now
    readonly property real nightLightUntil: adapter.nightLightUntil
    // Warmth in kelvin (6500 is neutral).
    readonly property int nightLightTemperature: adapter.nightLightTemperature

    function setDoNotDisturb(on) { adapter.doNotDisturb = on }
    function setNightLight(on) {
        now = Date.now();
        adapter.nightLightUntil = on ? QS.nightLightUntil(now) : 0;
    }

    // Re-evaluated when Night Light is due to end (and on toggles).
    property real now: Date.now()
    Timer {
        running: root.adapter_until > 0
        interval: Math.max(1000, Math.min(root.adapter_until - root.now, 3600 * 1000))
        onTriggered: root.now = Date.now()
    }
    readonly property real adapter_until: adapter.nightLightUntil

    // The compositor applies Night Light; tell it now and whenever it reconnects.
    onNightLightChanged: Compositor.nightLight(nightLight, nightLightTemperature)
    Connections {
        target: Compositor
        function onConnectedChanged() {
            if (Compositor.connected) Compositor.nightLight(root.nightLight, root.nightLightTemperature);
        }
    }

    readonly property string stateDir:
        (Quickshell.env("XDG_STATE_HOME") || Quickshell.env("HOME") + "/.local/state") + "/whiterose"

    FileView {
        path: root.stateDir + "/quick.json"
        onAdapterUpdated: writeAdapter()
        onLoadFailed: error => { if (error === FileViewError.FileNotFound) writeAdapter() }
        JsonAdapter {
            id: adapter
            property bool doNotDisturb: false
            property real nightLightUntil: 0
            property int nightLightTemperature: 4000
        }
    }
}
