// A modal card over the window (Figma: the network settings dialog).
// Esc or a click outside closes it. Content taller than the window scrolls
// between the fixed header and footer.
import QtQuick
import QtQuick.Controls
import qs

Popup {
    id: root
    property string title
    property string subtitle
    property string icon
    default property alias content: body.data
    property alias footer: foot.data
    modal: true
    focus: true
    anchors.centerIn: Overlay.overlay
    width: 520
    padding: 0
    closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside
    Overlay.modal: Rectangle { color: Qt.rgba(0, 0, 0, 0.55) }
    background: Rectangle { color: Theme.bg; radius: 18; border { width: 1; color: Theme.surface2 } }
    contentItem: Column {
        spacing: 0
        Item {
            width: root.width
            height: 72
            Rectangle {
                id: badge
                visible: root.icon !== ""
                x: 18; anchors.verticalCenter: parent.verticalCenter
                width: 38; height: 38; radius: 11
                color: Theme.rose
                Icon { anchors.centerIn: parent; name: root.icon; size: 18; color: Theme.onAccent }
            }
            Column {
                anchors { left: badge.visible ? badge.right : parent.left; leftMargin: badge.visible ? 12 : 20; verticalCenter: parent.verticalCenter }
                spacing: 2
                Text { text: root.title; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(15); weight: Font.DemiBold } }
                Text { visible: text !== ""; text: root.subtitle; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
            }
            IconButton {
                anchors { right: parent.right; rightMargin: 16; verticalCenter: parent.verticalCenter }
                width: 28; height: 28; icon: "x"; iconSize: 13
                onClicked: root.close()
            }
        }
        Flickable {
            id: scroller
            width: root.width
            // Room left in the window for the body (header 72, footer 59, 24 margin each side).
            readonly property real room: (root.Overlay.overlay?.height ?? 10000) - 72 - (foot.children.length > 0 ? 59 : 0) - 48
            height: Math.min(body.implicitHeight, Math.max(120, room))
            contentHeight: body.implicitHeight
            clip: contentHeight > height
            interactive: contentHeight > height
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar { policy: scroller.interactive ? ScrollBar.AsNeeded : ScrollBar.AlwaysOff }
            Column { id: body; x: 18; width: root.width - 36; spacing: 12; bottomPadding: 16 }
        }
        Rectangle { width: root.width; height: 1; color: Theme.surface2; visible: foot.children.length > 0 }
        Item {
            visible: foot.children.length > 0
            width: root.width; height: 58
            Row { id: foot; anchors { fill: parent; leftMargin: 18; rightMargin: 18 } spacing: 8 }
        }
    }
}
