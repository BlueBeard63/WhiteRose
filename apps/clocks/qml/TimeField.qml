import QtQuick
import WhiteRose.Kit

Rectangle {
    id: root
    property alias text: input.text
    property string label
    property int maximum: 59
    property alias editing: input.activeFocus
    function focusField() { input.forceActiveFocus(); input.selectAll() }
    width: 100; height: 72; radius: 10
    color: Theme.bg
    border { width: 1; color: input.activeFocus ? Theme.muted : Theme.surface2 }
    FontLoader { id: doto; source: Qt.resolvedUrl("../fonts/Doto.ttf") }
    TextInput {
        id: input
        anchors { fill: parent; margins: 8 }
        color: Theme.text
        horizontalAlignment: TextInput.AlignHCenter
        verticalAlignment: TextInput.AlignVCenter
        font { family: doto.name; pointSize: Theme.pt(44); weight: Font.Bold }
        selectByMouse: true
        maximumLength: 3
        validator: IntValidator { bottom: 0; top: root.maximum }
        selectionColor: Theme.rose
        selectedTextColor: Theme.onAccent
        Accessible.role: Accessible.EditableText
        Accessible.name: root.label
        onEditingFinished: if (text !== "") input.text = String(Number(input.text)).padStart(2, "0")
    }
}
