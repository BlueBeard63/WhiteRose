// Small round icon button (dock tabs, tray actions).
import QtQuick

Rectangle {
    id: root
    property string icon
    property real iconSize: 15
    property color iconColor: Theme.text
    property color fill: Theme.surface1
    property color hoverFill: Theme.surface2
    readonly property bool hovered: mouse.containsMouse
    // Its name (screen readers, tests); without one it isn't announced.
    property string label
    signal clicked()
    Accessible.role: Accessible.Button
    Accessible.name: root.label
    Accessible.ignored: root.label === ""
    Accessible.onPressAction: root.clicked()
    width: 32; height: 32; radius: width / 2
    color: mouse.containsMouse ? hoverFill : fill
    Behavior on color { ColorAnimation { duration: Theme.fastAnim } }
    scale: mouse.pressed ? 0.92 : 1
    Behavior on scale { NumberAnimation { duration: 90 } }
    Icon { anchors.centerIn: parent; name: root.icon; size: root.iconSize; color: root.iconColor }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
