// Secondary button used in toasts and panels (Figma: Button, surface fill).
import QtQuick

Rectangle {
    id: root
    property string icon: ""
    property string label: ""
    signal clicked()

    height: 32
    radius: 9
    color: mouse.containsMouse ? Theme.grip : Theme.surface2
    Behavior on color { ColorAnimation { duration: Theme.fastAnim } }

    Row {
        anchors.centerIn: parent
        spacing: 6
        Icon { anchors.verticalCenter: parent.verticalCenter; visible: root.icon !== ""; name: root.icon; size: 14 }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: root.label !== ""
            text: root.label
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(12.5); weight: Font.Medium }
        }
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
