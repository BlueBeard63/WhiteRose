// "Screenshot saved" toast (Figma: Screenshot - Saved (PRTSC)).
// Appears under the bar, flush with the right edge, after the compositor
// saves a screenshot. Hovering pauses the auto-dismiss.
import QtQuick
import Quickshell
import Quickshell.Wayland
import Quickshell.Widgets

PanelWindow {
    id: root

    // Current toast contents.
    property string path: ""
    property int shotWidth: 0
    property int shotHeight: 0
    property string error: ""
    readonly property bool failed: error !== ""

    property bool open: false
    readonly property int dismissAfter: 6000

    anchors { top: true; right: true }
    // Sits below the bar's reserved strip instead of covering it.
    exclusionMode: ExclusionMode.Normal
    implicitWidth: shape.width
    implicitHeight: content.implicitHeight
    color: "transparent"
    visible: open || slide.running

    WlrLayershell.namespace: "whiterose-toast"

    LayerTitle { name: "whiterose-toast" }
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None

    // The sliding item, not the shape inside it: a Region follows its own
    // item's moves only, so with `shape` it stayed where the slide started
    // (above the window) and every click went straight through.
    mask: Region { item: slider }

    function show() {
        open = true;
        progress.restart();
    }
    function dismiss() { open = false }

    Connections {
        target: Compositor
        function onScreenshotSaved(path, width, height) {
            root.error = "";
            root.path = path;
            root.shotWidth = width;
            root.shotHeight = height;
            root.show();
        }
        function onScreenshotFailed(error) {
            root.error = error;
            root.path = "";
            root.show();
        }
    }

    // Slides down out of the bar when opening, back up when closing.
    Item {
        id: slider
        width: shape.width
        height: shape.height
        y: root.open ? 0 : -height
        Behavior on y { NumberAnimation { id: slide; duration: 220; easing.type: Easing.OutCubic } }

        PanelShape {
            id: shape
            bodyWidth: 360
            bodyHeight: content.implicitHeight
            leftEar: true
            rightEar: false
            bottomLeftRadius: 28
            bottomRightRadius: 0
        }

        HoverHandler { id: hover }

        Column {
            id: content
            x: shape.bodyX + 18
            width: shape.bodyWidth - 36
            topPadding: 14
            bottomPadding: 14
            spacing: 12

            // Header: badge, title, size, time.
            Row {
                width: parent.width
                spacing: 10
                Rectangle {
                    width: 32; height: 32; radius: 10
                    color: root.failed ? Theme.tint(0.18) : Theme.rose
                    Icon {
                        anchors.centerIn: parent
                        size: 16
                        name: root.failed ? "alert" : "camera"
                        color: root.failed ? Theme.rose : Theme.onAccent
                    }
                }
                Column {
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - 32 - 10 - nowLabel.width - 10
                    spacing: 1
                    Text {
                        text: root.failed ? "Screenshot failed" : root.path === "" ? "Screenshot copied" : "Screenshot saved"
                        color: Theme.text
                        font { family: Theme.font; pointSize: Theme.pt(13.5); weight: Font.DemiBold }
                    }
                    Text {
                        width: parent.width
                        text: root.failed ? root.error : root.path === "" ? `On the clipboard · ${root.shotWidth} × ${root.shotHeight}` : `${root.shotWidth} × ${root.shotHeight}`
                        color: root.failed ? Theme.rose : Theme.muted
                        elide: Text.ElideRight
                        font { family: Theme.font; pointSize: Theme.pt(11.5) }
                    }
                }
                Text {
                    id: nowLabel
                    anchors.verticalCenter: parent.verticalCenter
                    text: "now"
                    color: Theme.faint
                    font { family: Theme.font; pointSize: Theme.pt(11) }
                }
            }

            // Thumbnail, in the screen's aspect ratio.
            ClippingRectangle {
                visible: !root.failed && root.path !== ""
                width: parent.width
                height: root.shotWidth > 0 ? Math.round(width * root.shotHeight / root.shotWidth) : 0
                radius: 10
                color: Theme.surface1
                border { width: 1; color: Theme.surface2 }
                Image {
                    anchors.fill: parent
                    source: root.path ? "file://" + root.path : ""
                    fillMode: Image.PreserveAspectFit
                    asynchronous: true
                    cache: false
                    // Decode at display size, not the full capture: far less memory.
                    sourceSize.width: width * 2
                }
            }

            // File path + "Copied" (the compositor put it on the clipboard).
            Row {
                visible: !root.failed && root.path !== ""
                width: parent.width
                spacing: 8
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - copied.width - 8
                    text: root.path.replace(Quickshell.env("HOME") ?? "\u0000", "~")
                    color: Theme.muted
                    elide: Text.ElideMiddle
                    font { family: Theme.mono; pointSize: Theme.pt(11) }
                }
                Rectangle {
                    id: copied
                    width: copiedRow.implicitWidth + 16
                    height: 22
                    radius: 7
                    color: Theme.tint(0.14)
                    Row {
                        id: copiedRow
                        anchors.centerIn: parent
                        spacing: 5
                        Icon { anchors.verticalCenter: parent.verticalCenter; name: "check"; size: 12; color: Theme.rose }
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: "Copied"
                            color: Theme.rose
                            font { family: Theme.font; pointSize: Theme.pt(11.5); weight: Font.Medium }
                        }
                    }
                }
            }

            // Actions (for a saved file).
            Row {
                visible: !root.failed && root.path !== ""
                width: parent.width
                spacing: 8
                ToastButton {
                    width: (parent.width - 8 * 2 - 34) / 2
                    icon: "open"
                    label: "Open"
                    onClicked: { Quickshell.execDetached(["xdg-open", root.path]); root.dismiss(); }
                }
                ToastButton {
                    width: (parent.width - 8 * 2 - 34) / 2
                    icon: "folder"
                    label: "Open folder"
                    onClicked: {
                        Quickshell.execDetached(["xdg-open", root.path.substring(0, root.path.lastIndexOf("/"))]);
                        root.dismiss();
                    }
                }
                ToastButton {
                    width: 34
                    icon: "trash"
                    name: "Delete"
                    onClicked: { Compositor.deleteScreenshot(root.path); root.dismiss(); }
                }
            }

            // Auto-dismiss: the line shrinks; hovering pauses it.
            Item {
                width: parent.width
                height: 2
                Rectangle { anchors.fill: parent; color: Theme.surface2; radius: 1 }
                Rectangle {
                    id: remaining
                    height: parent.height
                    radius: 1
                    color: Theme.pearl
                    opacity: 0.6
                    property real fraction: 1
                    width: parent.width * fraction
                    NumberAnimation on fraction {
                        id: progress
                        from: 1; to: 0
                        duration: root.dismissAfter
                        running: false
                        paused: running && hover.hovered
                        onFinished: root.dismiss()
                    }
                }
            }
        }
    }
}
