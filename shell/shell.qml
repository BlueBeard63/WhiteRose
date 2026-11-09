// WhiteRose shell entry point: `qs -p shell` (the compositor starts it with --shell).
import QtQuick
import Quickshell

ShellRoot {
    // The wallpaper, behind everything.
    Background {}

    // One bar per monitor.
    Variants {
        model: Quickshell.screens
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

    // Screenshot toast, on the primary screen.
    ScreenshotToast { screen: Quickshell.screens[0] ?? null }
}
