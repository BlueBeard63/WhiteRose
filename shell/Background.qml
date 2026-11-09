// The wallpaper, behind everything on each screen (layer-shell background).
// A gradient costs nothing to load; an image is decoded once at the
// screen's size. Changes to the next one on a timer if you asked for that.
import QtQuick
import QtQuick.Shapes
import Quickshell
import Quickshell.Wayland
import "Wallpapers.js" as W

Scope {
    Variants {
        model: Quickshell.screens
        delegate: PanelWindow {
            required property var modelData
            screen: modelData
            anchors { top: true; bottom: true; left: true; right: true }
            exclusionMode: ExclusionMode.Ignore
            color: Theme.bg
            WlrLayershell.layer: WlrLayer.Background
            WlrLayershell.namespace: "whiterose-wallpaper"
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.None

            WallpaperView {
                anchors.fill: parent
                wallpaper: Wallpaper.current
                // Decode at the screen's real size, not the file's.
                sourceSize: Qt.size(Math.ceil(modelData.width * (modelData.devicePixelRatio ?? 1)), Math.ceil(modelData.height * (modelData.devicePixelRatio ?? 1)))
            }
        }
    }

    Timer {
        interval: Math.max(1, Wallpaper.rotateMinutes) * 60000
        running: Wallpaper.rotateMinutes > 0
        repeat: true
        onTriggered: Wallpaper.next()
    }
}
