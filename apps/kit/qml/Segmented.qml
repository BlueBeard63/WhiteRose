// A row of options, one selected (Figma: Scale 100% / 125% / ...).
import QtQuick


Rectangle {
    id: root
    property var options: []   // [{ label, value }]
    property var value
    signal picked(var value)
    implicitWidth: row.implicitWidth + 6
    implicitHeight: 30
    width: implicitWidth; height: implicitHeight
    radius: 10
    anchors.verticalCenter: parent?.verticalCenter
    color: Theme.surface2
    Row {
        id: row
        anchors.centerIn: parent
        spacing: 2
        Repeater {
            model: root.options
            delegate: Rectangle {
                required property var modelData
                readonly property bool on: modelData.value === root.value
                width: segText.implicitWidth + 22; height: 24; radius: 8
                color: on ? Theme.pearl : segMouse.containsMouse ? Theme.grip : "transparent"
                Text {
                    id: segText
                    anchors.centerIn: parent
                    text: modelData.label
                    color: parent.on ? Theme.bg : Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(12); weight: Font.Medium }
                }
                MouseArea {
                    id: segMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: if (!parent.on) root.picked(modelData.value)
                }
            }
        }
    }
}
