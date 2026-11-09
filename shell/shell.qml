// WhiteRose shell entry point: `qs -p shell` (the compositor starts it with --shell).
import Quickshell

ShellRoot {
    // One bar per monitor.
    Variants {
        model: Quickshell.screens
        delegate: Bar {}
    }

    // ALT+TAB / SUPER+ALT+TAB, on the primary screen.
    Switcher { screen: Quickshell.screens[0] ?? null }

    // Screenshot toast, on the primary screen.
    ScreenshotToast { screen: Quickshell.screens[0] ?? null }
}
