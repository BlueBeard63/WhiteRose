// WhiteRose shell entry point: `qs -p shell` (the compositor starts it with --shell).
import QtQuick
import Quickshell

ShellRoot {
    // The wallpaper, behind everything.
    Background {}

    // One bar per monitor, or only on the main one (Settings > Top Bar):
    // the built-in screen, else the first.
    readonly property var mainScreen: {
        const first = Compositor.outputs.find(o => o.enabled);
        return Quickshell.screens.find(s => s.name === first?.connector) ?? Quickshell.screens[0] ?? null;
    }
    Variants {
        model: Daemon.setting("bar.screens", "all") === "main" && mainScreen ? [mainScreen] : Quickshell.screens
        delegate: Bar {}
    }

    // Tap SUPER: the island morphs into the launcher.
    Connections {
        target: Compositor
        function onAction(name) {
            if (name === "app_launcher") ShellState.toggle("launcher");
            else if (name === "quick_settings") ShellState.togglePanel("quick");
            else if (name === "notifications") ShellState.togglePanel("notifications");
            else if (name === "lock") ShellState.lock();
        }
    }

    LockScreen {}

    // Volume, brightness and media keys.
    MediaKeys {}

    // Each monitor's number and name, when asked from Settings > Displays.
    IdentifyOverlay {}

    // The snap layout picker (SUPER+Z, a drag to the top, a maximize button).
    SnapPicker {}

    // Snap zones edited on screen (Settings › Snapping, "Edit on screen").
    ZonesOverlay {}

    // First login: offer the app setup (Settings > Apps) once.
    Connections {
        target: Daemon
        function onSettingsChanged() {
            if (firstRun.done || Object.keys(Daemon.settings).length === 0) return;
            firstRun.done = true;
            if (!Daemon.setting("apps.setupDone", true)) firstRun.start();
        }
    }
    // A moment after login, so the desktop is up first.
    Timer { id: firstRun; property bool done: false; interval: 2500; onTriggered: ShellState.openSettings("apps") }

    // Password prompts when something needs your permission (polkit).
    PolkitPrompt {}

    // SUPER+SHIFT+S: the screenshot tool.
    ScreenshotOverlay {}

    // Screenshot toast, on the primary screen.
    ScreenshotToast { screen: Quickshell.screens[0] ?? null }
}
