// One key of a shortcut (Figma: the mono key caps, "SUPER", "TAB").
import QtQuick
import qs

Rectangle {
    property alias text: label.text
    implicitWidth: Math.max(22, label.implicitWidth + 12)
    implicitHeight: 22
    width: implicitWidth; height: implicitHeight
    radius: 5
    color: Theme.surface2
    border { width: 1; color: Theme.grip }
    anchors.verticalCenter: parent?.verticalCenter
    Text {
        id: label
        anchors.centerIn: parent
        color: Theme.muted
        font { family: Theme.mono; pointSize: Theme.pt(9.5) }
    }
}
