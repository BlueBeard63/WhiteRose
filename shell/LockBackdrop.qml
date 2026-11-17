// Behind the lock screen: the wallpaper, blurred and dimmed, like looking
// through frosted glass (no apps: the lock shows only its own surface).
// Loaded by LockScreen.qml through a Loader, so a missing QtQuick.Effects
// leaves the plain background rather than no lock screen.
import QtQuick
import QtQuick.Effects

Item {
    id: root
    anchors.fill: parent

    WallpaperView {
        id: wallpaper
        anchors.fill: parent
        visible: false
        wallpaper: Wallpaper.current
        // Blurred anyway: decode it small.
        sourceSize: Qt.size(Math.ceil(root.width / 2), Math.ceil(root.height / 2))
    }
    MultiEffect {
        anchors.fill: parent
        source: wallpaper
        autoPaddingEnabled: false
        blurEnabled: true
        blur: 1.0
        blurMax: 64
    }
    Rectangle {
        anchors.fill: parent
        color: Theme.bg
        opacity: 0.55
    }
}
