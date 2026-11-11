// A text button: "primary" (pearl), "danger" (rose text) or plain.
import QtQuick
import qs

Rectangle {
    id: root
    property string text
    property string icon
    property string kind: "plain"   // plain | primary | danger | ghost
    property bool enabled: true
    signal clicked()
    Accessible.role: Accessible.Button
    Accessible.name: root.text || root.icon
    Accessible.onPressAction: if (root.enabled) root.clicked()
    implicitWidth: row.implicitWidth + 22
    implicitHeight: 30
    width: implicitWidth; height: implicitHeight
    radius: 9
    anchors.verticalCenter: parent?.verticalCenter
    opacity: enabled ? 1 : 0.4
    color: kind === "primary" ? (mouse.containsMouse ? Qt.lighter(Theme.pearl, 1.04) : Theme.pearl)
         : kind === "ghost" ? (mouse.containsMouse ? Theme.surface2 : "transparent")
         : mouse.containsMouse ? Theme.grip : Theme.surface2
    Row {
        id: row
        anchors.centerIn: parent
        spacing: 6
        Icon {
            visible: root.icon !== ""
            anchors.verticalCenter: parent.verticalCenter
            name: root.icon; size: 13
            color: label.color
        }
        Text {
            id: label
            anchors.verticalCenter: parent.verticalCenter
            text: root.text
            color: root.kind === "primary" ? Theme.bg : root.kind === "danger" ? Theme.rose : Theme.text
            font { family: Theme.font; pointSize: Theme.pt(12); weight: Font.Medium }
        }
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        enabled: root.enabled
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
