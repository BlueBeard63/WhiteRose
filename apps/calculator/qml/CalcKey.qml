// The kit's Button supplies pointer/accessibility behavior. A separate label
// lets the keypad use larger type without changing every app's buttons.
import QtQuick
import WhiteRose.Kit

Button {
    id: key
    property string label
    readonly property bool digit: /^[0-9.]$/.test(label)
    property real textSize: digit ? 22 : label.length > 2 ? 13 : 18
    text: ""
    anchors.verticalCenter: undefined
    implicitHeight: 48
    Accessible.name: label === "⌫" ? "Backspace" : label === "=" ? "Equals" : label
    color: label === "=" ? (hover.hovered ? Qt.lighter(Theme.rose, 1.08) : Theme.rose)
        : hover.hovered ? Theme.grip : digit ? Theme.surface1 : Theme.surface2
    HoverHandler { id: hover }
    Text {
        anchors.centerIn: parent
        text: key.label
        color: key.label === "=" ? Theme.onAccent : Theme.text
        font { family: Theme.font; pointSize: Theme.pt(key.textSize); weight: Font.Medium }
    }
}
