// A 0-1 level with a pearl fill (volume, ...). Drag, click or scroll.
import QtQuick
import qs

Item {
    id: root
    property real value: 0
    property real maximum: 1
    property bool dimmed: false
    signal moved(real value)
    implicitWidth: 220
    implicitHeight: 20
    width: implicitWidth; height: implicitHeight
    anchors.verticalCenter: parent?.verticalCenter
    function setFrom(x) { root.moved(Math.max(0, Math.min(root.maximum, x / width * root.maximum))) }
    Rectangle {
        anchors.verticalCenter: parent.verticalCenter
        width: parent.width; height: 6; radius: 3
        color: Theme.surface2
        Rectangle {
            width: parent.width * Math.min(1, root.value / root.maximum); height: parent.height; radius: 3
            color: root.dimmed ? Theme.grip : Theme.pearl
        }
    }
    Rectangle {
        x: Math.min(1, root.value / root.maximum) * (root.width - width)
        anchors.verticalCenter: parent.verticalCenter
        width: 14; height: 14; radius: 7
        color: root.dimmed ? Theme.muted : Theme.pearl
        border { width: 2; color: Theme.surface1 }
    }
    MouseArea {
        anchors.fill: parent
        anchors.margins: -6
        cursorShape: Qt.PointingHandCursor
        preventStealing: true
        onPressed: m => root.setFrom(m.x - 6)
        onPositionChanged: m => { if (pressed) root.setFrom(m.x - 6) }
        onWheel: w => root.moved(Math.max(0, Math.min(root.maximum, root.value + (w.angleDelta.y > 0 ? 0.05 : -0.05))))
    }
}
