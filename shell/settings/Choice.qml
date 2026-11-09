// A dropdown: shows the current option; click for the list.
// options: [{ label, value }] (or plain strings).
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
        y: root.height + 4
        x: root.width - width
        width: Math.max(root.width, list.implicitWidth + 12)
        padding: 6
        background: Rectangle { color: Theme.surface1; radius: 12; border { width: 1; color: Theme.surface2 } }
        contentItem: Column {
            id: list
            spacing: 2
            Repeater {
                model: root.items
                delegate: Rectangle {
                    required property var modelData
                    width: Math.max(menu.width - 12, optText.implicitWidth + 40)
                    height: 30
                    radius: 8
                    color: optMouse.containsMouse ? Theme.surface2 : "transparent"
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
                    MouseArea {
                        id: optMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: { menu.close(); if (modelData.value !== root.value) root.picked(modelData.value) }
                    }
                }
            }
        }
    }
}
