// Draws one wallpaper: a built-in gradient or an image (cropped to fill).
// Used for the desktop and for the thumbnails in Settings.
import QtQuick
import QtQuick.Shapes
import "Wallpapers.js" as W

Item {
    id: root
    property string wallpaper: "builtin:rose"
    property size sourceSize: Qt.size(0, 0)
    readonly property var gradient: W.builtin(root.wallpaper) ?? (image.status === Image.Error ? W.builtin("builtin:rose") : null)
    clip: true

    Shape {
        anchors.fill: parent
        visible: root.gradient !== null
        preferredRendererType: Shape.CurveRenderer
        ShapePath {
            strokeWidth: -1
            fillGradient: LinearGradient {
                x1: 0; y1: 0; x2: root.width; y2: root.height
                GradientStop { position: 0; color: root.gradient?.from ?? "black" }
                GradientStop { position: 0.55; color: root.gradient?.via ?? "black" }
                GradientStop { position: 1; color: root.gradient?.to ?? "black" }
            }
            startX: 0; startY: 0
            PathLine { x: root.width; y: 0 }
            PathLine { x: root.width; y: root.height }
            PathLine { x: 0; y: root.height }
            PathLine { x: 0; y: 0 }
        }
    }
    Image {
        id: image
        anchors.fill: parent
        visible: !W.isBuiltin(root.wallpaper) && status === Image.Ready
        source: W.isBuiltin(root.wallpaper) ? "" : "file://" + root.wallpaper
        sourceSize: root.sourceSize
        fillMode: Image.PreserveAspectCrop
        asynchronous: true
        cache: false
    }
}
