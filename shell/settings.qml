// Its Wayland app id is its desktop entry's name, so the dock, switcher and
// launcher show Settings with its icon (not QuickShell's "org.quickshell").
//@ pragma AppId whiterose-settings
// WhiteRose Settings: `qs -p shell/settings.qml` (bin/whiterose-settings
// starts it, or brings the open one to a page). A separate window that exits
// when closed, so it costs nothing while you're not using it.
//   qs -p shell/settings.qml ipc call settings open network
import QtQuick
import Quickshell
import qs.settings

ShellRoot {
    SettingsWindow {}
}
