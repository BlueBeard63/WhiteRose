// A row of options, one selected (Figma: Scale 100% / 125% / ...). Options
// with an `icon` can be shown as icons only (`iconOnly`): their label is
// then their name (screen readers, tests) and a tooltip on hover.
import QtQuick
import qs

Rectangle {
    id: root
    property var options: []   // [{ label, value, icon? }]
    property bool iconOnly: false
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
                id: seg
                required property var modelData
                readonly property bool on: modelData.value === root.value
                readonly property bool glyph: root.iconOnly && !!modelData.icon
                Accessible.role: Accessible.RadioButton
                Accessible.name: modelData.label ?? ""
                Accessible.checkable: true
                Accessible.checked: on
                Accessible.onPressAction: if (!on) root.picked(modelData.value)
                width: glyph ? 36 : segText.implicitWidth + 22; height: 24; radius: 8
                color: on ? Theme.pearl : segMouse.containsMouse ? Theme.grip : "transparent"
                Text {
                    id: segText
                    visible: !seg.glyph
                    anchors.centerIn: parent
                    text: modelData.label
                    color: parent.on ? Theme.bg : Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(12); weight: Font.Medium }
                }
                Icon {
                    visible: seg.glyph
                    anchors.centerIn: parent
                    name: modelData.icon ?? ""
                    size: 16
                    color: seg.on ? Theme.bg : Theme.muted
                }
                // An icon's name, while the pointer rests on it.
                Rectangle {
                    visible: seg.glyph && segMouse.containsMouse
                    z: 10
                    anchors { horizontalCenter: parent.horizontalCenter; top: parent.bottom; topMargin: 8 }
                    width: tipText.implicitWidth + 16; height: 24; radius: 7
                    color: Theme.surface2
                    border { width: 1; color: Theme.grip }
                    Text {
                        id: tipText
                        anchors.centerIn: parent
                        text: modelData.label
                        color: Theme.text
                        font { family: Theme.font; pointSize: Theme.pt(11) }
                    }
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
