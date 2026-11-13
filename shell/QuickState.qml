// Quick settings toggles owned by the shell, kept in the settings database:
// - Do not disturb (quick.doNotDisturb): the notification server holds popups.
// - Night light (quick.nightLight.until / .temperature): the compositor warms
//   the screen; it switches itself off at the next 07:00.
pragma Singleton
import QtQuick
import Quickshell
import "QuickSettings.js" as QS

Singleton {
    id: root

    readonly property bool doNotDisturb: Daemon.setting("quick.doNotDisturb", false)
    readonly property real nightLightUntil: Daemon.setting("quick.nightLight.until", 0)
    readonly property bool nightLight: nightLightUntil > now
    // Warmth in kelvin (6500 is neutral).
    readonly property int nightLightTemperature: Daemon.setting("quick.nightLight.temperature", 4000)

    function setDoNotDisturb(on) { Daemon.set("quick.doNotDisturb", on) }
    function setNightLight(on) {
        now = Date.now();
        Daemon.set("quick.nightLight.until", on ? QS.nightLightUntil(now) : 0);
    }

    // Re-evaluated when Night light is due to end (and on toggles).
    property real now: Date.now()
    Timer {
        running: root.nightLightUntil > 0
        interval: Math.max(1000, Math.min(root.nightLightUntil - root.now, 3600 * 1000))
        onTriggered: root.now = Date.now()
    }

    // The compositor applies Night light; tell it now and whenever it reconnects.
    onNightLightChanged: Compositor.nightLight(nightLight, nightLightTemperature)
    onNightLightTemperatureChanged: if (nightLight) Compositor.nightLight(true, nightLightTemperature)
    Connections {
        target: Compositor
        function onConnectedChanged() {
            if (Compositor.connected) Compositor.nightLight(root.nightLight, root.nightLightTemperature);
        }
    }
}
