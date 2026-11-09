// Switches the island between its dock sections: apps, clipboard, screenshots.
import QtQuick

Row {
    id: root
    spacing: 4
    readonly property var tabs: [["dock", "grid"], ["clipboard", "clipboard"], ["screenshots", "image"]]
    Repeater {
        model: root.tabs
        delegate: Rectangle {
            required property var modelData
            readonly property bool active: ShellState.islandView === modelData[0]
            width: 34; height: 30; radius: 10
            color: active ? Theme.pearl : tabMouse.containsMouse ? Theme.surface2 : Theme.surface1
            Behavior on color { ColorAnimation { duration: Theme.fastAnim } }
            Icon { anchors.centerIn: parent; name: modelData[1]; size: 15; color: parent.active ? Theme.bg : Theme.text }
            MouseArea {
                id: tabMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: ShellState.show(modelData[0])
            }
        }
    }
}
