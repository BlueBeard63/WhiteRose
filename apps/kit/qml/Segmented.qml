// A row of options, one selected (Figma: Scale 100% / 125% / ...).
import QtQuick
import QtQuick.Controls


Rectangle {
    id: root
    property var options: []   // [{ label, value }] or [{ icon, value, label (a tooltip) }]
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
                // (For screen readers and the UI tests: one of a set.)
                Accessible.role: Accessible.RadioButton
                Accessible.name: modelData.label ?? modelData.icon ?? ""
                Accessible.checkable: true
                Accessible.checked: on
                Accessible.onPressAction: if (!on) root.picked(modelData.value)
                width: modelData.icon ? 30 : segText.implicitWidth + 22; height: 24; radius: 8
                color: on ? Theme.pearl : segMouse.containsMouse ? Theme.grip : "transparent"
                Icon {
                    visible: !!modelData.icon
                    anchors.centerIn: parent
                    name: modelData.icon ?? ""
                    size: 14
                    color: parent.on ? Theme.bg : Theme.muted
                }
                Text {
                    id: segText
                    visible: !modelData.icon
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
                ToolTip.visible: !!modelData.icon && segMouse.containsMouse
                ToolTip.delay: 500
                ToolTip.text: modelData.label ?? ""
            }
        }
    }
}
