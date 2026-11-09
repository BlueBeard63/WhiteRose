// One live window preview (dock hover, later the window switcher): the
// compositor copies the window's pixels via hyprland-toplevel-export, only
// while this is visible and only when the window actually redraws.
import QtQuick
import Quickshell
import Quickshell.Wayland
import Quickshell.Widgets

Item {
    id: root
    required property var toplevel
    property real maxWidth: 220
    property real maxHeight: 138
    // Outlined as the current choice (switcher selection).
    property bool highlighted: false
    property bool showTitle: true
    signal activated()

    readonly property bool hovered: mouse.containsMouse || closeMouse.containsMouse

    width: frame.width
    height: frame.height + (showTitle ? 6 + title.height : 0)

    ClippingRectangle {
        id: frame
        // Keep the window's aspect ratio inside maxWidth x maxHeight.
        readonly property size src: view.sourceSize
        readonly property real k: src.width > 0 ? Math.min(root.maxWidth / src.width, root.maxHeight / src.height) : 1
        width: src.width > 0 ? Math.round(src.width * k) : root.maxWidth
        height: src.height > 0 ? Math.round(src.height * k) : root.maxHeight * 0.62
        radius: 10
        color: Theme.surface1
        border {
            width: root.highlighted ? 2 : root.toplevel?.activated ? 1.5 : 1
            color: root.highlighted || mouse.containsMouse ? Theme.pearl : Theme.surface2
        }

        ScreencopyView {
            id: view
            anchors.fill: parent
            captureSource: root.visible ? root.toplevel : null
            live: root.visible
            paintCursor: false
            constraintSize: Qt.size(root.maxWidth * 2, root.maxHeight * 2)
        }

        // Until the first frame arrives (or if capture isn't available).
        Text {
            anchors.centerIn: parent
            visible: !view.hasContent
            text: "…"
            color: Theme.faint
            font { family: Theme.font; pointSize: Theme.pt(14) }
        }

        MouseArea {
            id: mouse
            anchors.fill: parent
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: root.activated()
        }

        // Close button, shown on hover.
        Rectangle {
            anchors { top: parent.top; right: parent.right; margins: 6 }
            width: 22; height: 22; radius: 11
            visible: root.hovered
            color: closeMouse.containsMouse ? Theme.rose : Qt.rgba(0.06, 0.06, 0.06, 0.85)
            Icon {
                anchors.centerIn: parent
                name: "x"; size: 12
                color: closeMouse.containsMouse ? Theme.bg : Theme.text
            }
            MouseArea {
                id: closeMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: root.toplevel.close()
            }
        }
    }

    Text {
        id: title
        visible: root.showTitle
        anchors { top: frame.bottom; topMargin: 6; horizontalCenter: frame.horizontalCenter }
        width: frame.width
        horizontalAlignment: Text.AlignHCenter
        text: root.toplevel?.title ?? ""
        elide: Text.ElideRight
        color: root.toplevel?.activated ? Theme.text : Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(11.5); weight: Font.Medium }
    }
}
