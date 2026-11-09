// The system state that comes from whiterosed rather than a QuickShell
// service: screen brightness (set through logind), airplane mode (rfkill),
// and logind's lock/unlock requests for this session.
// `whiterosed watch` stays running and prints a JSON line on every change; it
// sleeps in the kernel in between, so nothing here polls.
pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

Singleton {
    id: root

    readonly property bool available: watcher.running
    // 0-100, or -1 without a backlight (desktop monitors).
    property int brightness: -1
    property bool airplane: false
    property int radios: 0
    // Current weather for the place in weather.toml ({ temperature, unit,
    // icon, description, place, updated }), or null when not set up.
    property var weather: null
    // Bumps whenever the calendars might have changed (re-read events).
    property real calendars: 0

    // Called continuously while a slider drags: only the latest value is
    // sent, at most every 50 ms, one process at a time.
    property int pendingBrightness: -1
    function setBrightness(percent) {
        pendingBrightness = Math.max(1, Math.min(100, Math.round(percent)));
        brightness = pendingBrightness; // move the slider now, the watch confirms
        if (!setter.running && !throttle.running) flush();
    }
    function flush() {
        if (pendingBrightness < 0) return;
        setter.command = ["whiterosed", "brightness", "set", String(pendingBrightness)];
        pendingBrightness = -1;
        setter.running = true;
        throttle.restart();
    }
    Timer { id: throttle; interval: 50; onTriggered: if (!setter.running) root.flush() }
    Process {
        id: setter
        onExited: if (!throttle.running) root.flush()
    }

    function setAirplane(on) {
        airplane = on;
        Quickshell.execDetached(["whiterosed", "airplane", on ? "on" : "off"]);
    }

    Process {
        id: watcher
        command: ["whiterosed", "watch"]
        running: true
        stdout: SplitParser {
            onRead: line => {
                try {
                    const s = JSON.parse(line);
                    // logind asked to lock or unlock (loginctl, idle, lid).
                    if (s.event === "lock") { ShellState.lock(); return; }
                    if (s.event === "unlock") { ShellState.locked = false; return; }
                    root.brightness = s.brightness ?? -1;
                    root.airplane = s.airplane;
                    root.radios = s.radios;
                    root.weather = s.weather ?? null;
                    root.calendars = s.calendars ?? 0;
                } catch (e) {
                    console.warn("whiterosed watch:", e);
                }
            }
        }
        // whiterosed restarted or crashed: start watching again shortly.
        onExited: (code, status) => {
            console.warn("whiterosed watch exited with", code, "- restarting");
            restart.start();
        }
    }
    Timer { id: restart; interval: 3000; onTriggered: watcher.running = true }
}
