// A text box. `secret` hides what's typed (with an eye to show it).
import QtQuick
import qs

Rectangle {
    id: root
    property alias text: input.text
    property string placeholder
    property bool secret: false
    property bool mono: false
    property bool error: false
    property bool reveal: false
    // Its name (screen readers, tests): the row's title in a SettingRow,
    // else the placeholder.
    property string label: parent?.rowTitle ?? ""
    signal accepted()
    // Enter pressed or the field left.
    signal editingDone()
    function focusField() { input.forceActiveFocus() }
    implicitWidth: 260
    implicitHeight: 36
    width: implicitWidth; height: implicitHeight
    radius: 10
    color: Theme.bg
    border { width: 1; color: root.error ? Theme.rose : input.activeFocus ? Theme.muted : Theme.surface2 }
    TextInput {
        id: input
        Accessible.role: Accessible.EditableText
        Accessible.name: root.label || root.placeholder
        // (Not Accessible.passwordEdit: Qt then drops the name. The echo
        // mode already keeps what's typed out of the tree, as dots.)
        anchors { fill: parent; leftMargin: 12; rightMargin: root.secret ? 34 : 12 }
        verticalAlignment: TextInput.AlignVCenter
        clip: true
        color: Theme.text
        selectionColor: Theme.rose
        selectedTextColor: Theme.bg
        echoMode: root.secret && !root.reveal ? TextInput.Password : TextInput.Normal
        passwordCharacter: "•"
        font { family: root.mono ? Theme.mono : Theme.font; pointSize: Theme.pt(12.5) }
        onAccepted: root.accepted()
        onEditingFinished: root.editingDone()
        Text {
            anchors.verticalCenter: parent.verticalCenter
            visible: input.text === "" && !input.activeFocus
            text: root.placeholder
            color: Theme.faint
            font: input.font
        }
    }
    Icon {
        visible: root.secret
        anchors { right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
        name: root.reveal ? "eyeOff" : "eye"; size: 15; color: Theme.muted
        MouseArea { anchors.fill: parent; anchors.margins: -6; cursorShape: Qt.PointingHandCursor; onClicked: root.reveal = !root.reveal }
    }
}
