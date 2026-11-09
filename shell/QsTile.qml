// A quick settings toggle (Figma: Toggle/*): round icon, title, status line.
// Active tiles fill the icon circle with rose.
import QtQuick

Rectangle {
    id: root
    property string icon
    property string title
    property string subtitle
    property bool active: false
    signal clicked()

    implicitWidth: 158
    implicitHeight: 56
    radius: 16
    color: mouse.containsMouse ? Theme.surface2 : Theme.surface1
    Behavior on color { ColorAnimation { duration: Theme.fastAnim } }
    scale: mouse.pressed ? 0.97 : 1
    Behavior on scale { NumberAnimation { duration: 90 } }

    Rectangle {
        id: badge
        x: 10
        anchors.verticalCenter: parent.verticalCenter
        width: 34; height: 34; radius: 17
        color: root.active ? Theme.rose : Theme.surface2
        Behavior on color { ColorAnimation { duration: Theme.fastAnim } }
        Icon {
            anchors.centerIn: parent
            name: root.icon; size: 15
            color: root.active ? Theme.onAccent : Theme.text
        }
    }
    Column {
        anchors { left: badge.right; leftMargin: 10; right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
        spacing: 1
        Text {
            width: parent.width
            text: root.title
            elide: Text.ElideRight
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.Medium }
        }
        Text {
            width: parent.width
            visible: text !== ""
            text: root.subtitle
            elide: Text.ElideRight
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(11) }
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
