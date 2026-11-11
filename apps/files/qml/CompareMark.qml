// Comparing two folders (the Sheet's Compare): a dot on what differs, in
// the colour the Sheet lists it with (hover: what it means).
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit

Rectangle {
    id: root
    // [colour, what it means] (from the Sheet's Compare), or "".
    property var look: null
    property real size: 9
    visible: !!root.look
    width: root.size; height: root.size; radius: root.size / 2
    color: root.look ? root.look[0] : "transparent"
    border { width: 1.5; color: Theme.bg }
    MouseArea { id: m; anchors { fill: parent; margins: -4 } hoverEnabled: true; acceptedButtons: Qt.NoButton }
    ToolTip.visible: m.containsMouse && !!root.look
    ToolTip.text: root.look ? root.look[1] : ""
    ToolTip.delay: 300
}
