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
