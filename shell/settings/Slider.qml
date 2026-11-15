// A 0-1 level with a pearl fill (volume, ...). Drag or click; the wheel
// moves it only once it's been clicked (until the pointer leaves it), so
// scrolling a page past a slider scrolls the page.
import QtQuick
import qs

Item {
    id: root
    property real value: 0
    property real maximum: 1
    // How far one turn of the wheel moves it.
    property real step: 0.05
    property real wheelRest: 0
    // Clicked or dragged, and the pointer still over it: the wheel moves it.
    property bool engaged: false
    property bool dimmed: false
    property string label: parent?.rowTitle ?? ""
    signal moved(real value)
    Accessible.role: Accessible.Slider
    Accessible.name: root.label
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
        hoverEnabled: true
        onExited: root.engaged = false
        onPressed: m => { root.engaged = true; root.setFrom(m.x - 6) }
        onPositionChanged: m => { if (pressed) root.setFrom(m.x - 6) }
        // A step a notch (120); a touchpad's fractions add up until they
        // make a step on a coarse slider (else rounding would eat them).
        onWheel: w => {
            // Not clicked yet: the wheel is the page's.
            if (!root.engaged) { w.accepted = false; return }
            const n = w.angleDelta.y / 120;
            if (n === 0) return;
            root.wheelRest += n * root.step;
            if (root.step > 0.05 && Math.abs(root.wheelRest) < root.step - 1e-9) return;
            root.moved(Math.max(0, Math.min(root.maximum, root.value + root.wheelRest)));
            root.wheelRest = 0;
        }
    }
}
