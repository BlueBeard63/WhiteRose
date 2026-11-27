// Same 146x18 content box as the approved Desktop - Idle copies (203:2284).
// The island owns clicks and its unchanged 170x18 outline.
import QtQuick

Item {
    id: root
    property var entry: Clocks.readout
    width: 146; height: 18
    Accessible.role: Accessible.Button
    Accessible.name: entry ? "Clocks: " + entry.label : ""
    Accessible.description: entry ? entry.view + ": " + entry.value : ""
    Accessible.onPressAction: if (root.entry) Clocks.open(root.entry.view)
    Row {
        anchors.fill: parent
        spacing: 6
        Icon {
            anchors.verticalCenter: parent.verticalCenter
            name: root.entry?.icon ?? "timer"; size: 12
            color: root.entry?.alert ? Theme.rose : Theme.text
        }
        Text {
            width: Math.max(0, root.width - 12 - value.width - (extra.visible ? extra.width + 6 : 0) - 12)
            anchors.verticalCenter: parent.verticalCenter
            text: root.entry?.label ?? ""
            textFormat: Text.PlainText
            elide: Text.ElideRight
            color: root.entry?.alert ? Theme.rose : Theme.text
            font { family: Theme.font; pointSize: Theme.pt(11) }
        }
        Text {
            id: value
            anchors.verticalCenter: parent.verticalCenter
            text: root.entry?.value ?? ""
            color: root.entry?.alert ? Theme.rose : Theme.text
            font { family: Theme.mono; pointSize: Theme.pt(11) }
        }
        Text {
            id: extra
            visible: text !== ""
            anchors.verticalCenter: parent.verticalCenter
            text: root.entry?.extra ?? ""
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(10) }
        }
    }
}
