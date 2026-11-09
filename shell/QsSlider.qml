// A quick settings level (Figma: Slider/*): a pearl fill over a dark track,
// the icon on the fill, the percentage at the right. Drag or click anywhere
// to set it, scroll to nudge; clicking the icon runs iconClicked (mute).
import QtQuick

Rectangle {
    id: root
    property string icon
    property real value: 0          // 0-1
    property real minimum: 0        // brightness never goes fully dark
    property bool dimmed: false     // e.g. muted: the fill greys out
    signal moved(real value)
    signal iconClicked()

    implicitWidth: 324
    implicitHeight: 34
    radius: 17
    color: Theme.surface1

    function setFrom(x) { root.moved(Math.max(root.minimum, Math.min(1, (x - 6) / (width - 12)))) }

    Rectangle {
        id: fill
        height: parent.height
        radius: parent.radius
        width: Math.max(height, root.width * Math.max(0, Math.min(1, root.value)))
        color: root.dimmed ? Theme.surface2 : Theme.pearl
        Behavior on color { ColorAnimation { duration: Theme.fastAnim } }
    }
    MouseArea {
        anchors.fill: parent
        cursorShape: Qt.PointingHandCursor
        preventStealing: true
        onPressed: mouse => root.setFrom(mouse.x)
        onPositionChanged: mouse => { if (pressed) root.setFrom(mouse.x) }
        onWheel: wheel => root.moved(Math.max(root.minimum, Math.min(1, root.value + (wheel.angleDelta.y > 0 ? 0.05 : -0.05))))
    }
    // The icon is its own button (mute) on top of the drag area.
    Item {
        x: 4; width: 32; height: parent.height
        Icon {
            anchors.centerIn: parent
            name: root.icon; size: 16
            color: root.dimmed ? Theme.muted : Theme.bg
        }
        MouseArea {
            anchors.fill: parent
            cursorShape: Qt.PointingHandCursor
            onClicked: root.iconClicked()
        }
    }
    Text {
        anchors { right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter }
        text: Math.round(root.value * 100) + "%"
        color: Theme.muted
        font { family: Theme.mono; pointSize: Theme.pt(11) }
    }
}
