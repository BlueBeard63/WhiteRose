// A small mono tag (Figma: "WPA3", "saved", "core").
import QtQuick
import qs

Rectangle {
    property alias text: label.text
    property color tint: Theme.surface2
    property color ink: Theme.muted
    implicitWidth: label.implicitWidth + 10
    implicitHeight: 16
    width: implicitWidth; height: implicitHeight
    radius: 5
    color: tint
    anchors.verticalCenter: parent?.verticalCenter
    Text {
        id: label
        anchors.centerIn: parent
        color: parent.ink
        font { family: Theme.mono; pointSize: Theme.pt(9.5) }
    }
}
