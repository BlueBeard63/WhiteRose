// Four bars for a Wi-Fi signal strength (0-1).
import QtQuick
import qs

Row {
    property real strength: 0
    property color color: Theme.text
    spacing: 2
    anchors.verticalCenter: parent?.verticalCenter
    Repeater {
        model: 4
        delegate: Rectangle {
            required property int index
            width: 3
            height: 4 + index * 3
            anchors.bottom: parent.bottom
            radius: 1
            color: parent.color
            opacity: parent.strength >= (index + 0.5) / 4.5 ? 1 : 0.25
        }
    }
}
