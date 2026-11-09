// WhiteRose shell entry point: `qs -p shell` (the compositor starts it with --shell).
import Quickshell

ShellRoot {
    // One bar per monitor.
    Variants {
        model: Quickshell.screens
        delegate: Bar {}
    }
}
