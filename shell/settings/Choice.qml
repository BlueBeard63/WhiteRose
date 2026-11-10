// A dropdown: shows the current option; click for the list. The list opens
// below, or above when there isn't room below, and always stays inside the
// window (scrolling when it's taller than that).
// options: [{ label, value, hint?, disabled? }] (or plain strings). `hint`
// is shown faintly after the label (e.g. "Unplugged"); disabled options
// can't be picked.
import QtQuick
import QtQuick.Controls
import qs

Rectangle {
    id: root
    property var options: []
    property var value
    property bool enabled: true
    signal picked(var value)
    readonly property var items: options.map(o => typeof o === "object" ? o : { label: String(o), value: o })
    readonly property string current: (items.find(o => o.value === value) ?? { label: "" }).label

    implicitWidth: Math.max(80, row.implicitWidth + 22)
    implicitHeight: 30
    width: implicitWidth; height: implicitHeight
    radius: 9
    anchors.verticalCenter: parent?.verticalCenter
    color: mouse.containsMouse || menu.opened ? Theme.grip : Theme.surface2
    opacity: enabled ? 1 : 0.4
    Row {
        id: row
        anchors.centerIn: parent
        spacing: 6
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.current
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(12); weight: Font.Medium }
        }
        Icon { anchors.verticalCenter: parent.verticalCenter; name: "chevDown"; size: 12; color: Theme.muted }
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        enabled: root.enabled
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: menu.open()
    }
    Popup {
        id: menu
        // Kept this far inside the window.
        margins: 8
        readonly property real full: list.implicitHeight + topPadding + bottomPadding
        readonly property real winH: root.Window.window ? root.Window.window.height : 10000
        readonly property real atY: opened || visible ? root.mapToItem(null, 0, 0).y : 0
        readonly property bool above: atY + root.height + 4 + full > winH - margins && atY - 4 - full >= margins
        height: Math.min(full, Math.max(80, above ? atY - 4 - margins : winH - margins - atY - root.height - 4))
        y: above ? -height - 4 : root.height + 4
        x: root.width - width
        width: Math.max(root.width, list.implicitWidth + 12)
        padding: 6
        background: Rectangle { color: Theme.surface1; radius: 12; border { width: 1; color: Theme.surface2 } }
        contentItem: Flickable {
            clip: true
            contentHeight: list.implicitHeight
            boundsBehavior: Flickable.StopAtBounds
            interactive: contentHeight > height
            Column {
                id: list
                spacing: 2
                Repeater {
                    model: root.items
                    delegate: Rectangle {
                        required property var modelData
                        width: Math.max(menu.width - 12, optText.implicitWidth + hintText.implicitWidth + 52)
                        height: 30
                        radius: 8
                        opacity: modelData.disabled ? 0.45 : 1
                        color: optMouse.containsMouse && !modelData.disabled ? Theme.surface2 : "transparent"
                        Icon {
                            anchors { left: parent.left; leftMargin: 8; verticalCenter: parent.verticalCenter }
                            visible: modelData.value === root.value
                            name: "check"; size: 12; color: Theme.rose
                        }
                        Text {
                            id: optText
                            anchors { left: parent.left; leftMargin: 28; verticalCenter: parent.verticalCenter }
                            text: modelData.label
                            color: Theme.text
                            font { family: Theme.font; pointSize: Theme.pt(12) }
                        }
                        Text {
                            id: hintText
                            anchors { left: optText.right; leftMargin: 8; verticalCenter: parent.verticalCenter }
                            text: modelData.hint ?? ""
                            color: Theme.faint
                            font { family: Theme.font; pointSize: Theme.pt(11) }
                        }
                        MouseArea {
                            id: optMouse
                            anchors.fill: parent
                            enabled: !modelData.disabled
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: { menu.close(); if (modelData.value !== root.value) root.picked(modelData.value) }
                        }
                    }
                }
            }
        }
    }
}
