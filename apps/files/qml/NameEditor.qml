// Naming in place (a new folder or file, or one renamed): its name, typed
// right where it shows in the folder. Enter does it, Esc doesn't; clicking
// something else in the folder keeps what's typed. A name that can't be
// used goes red (hover: why) and stays for another try.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import "Format.js" as F

Rectangle {
    id: root
    required property var pane
    property bool dir: false
    property real fontSize: 12.5
    property int align: TextInput.AlignLeft
    readonly property bool wrong: root.pane.namingError !== ""
    implicitHeight: input.implicitHeight + 8
    radius: 6
    color: Theme.bg
    border { width: 1; color: root.wrong ? Theme.rose : Theme.tint(0.7) }
    function take() { input.forceActiveFocus() }
    Component.onCompleted: root.pane.nameEditor = root
    Component.onDestruction: if (root.pane && root.pane.nameEditor === root) root.pane.nameEditor = null
    TextInput {
        id: input
        Accessible.role: Accessible.EditableText
        Accessible.name: "Name"
        anchors { fill: parent; leftMargin: 6; rightMargin: 6; topMargin: 4; bottomMargin: 4 }
        horizontalAlignment: root.align
        verticalAlignment: TextInput.AlignVCenter
        clip: true
        selectByMouse: true
        text: root.pane.namingText
        color: Theme.text
        selectionColor: Theme.tint(0.4)
        font { family: Theme.font; pointSize: Theme.pt(root.fontSize) }
        Component.onCompleted: {
            // (Once it's in the window: before that, focus doesn't take.)
            Qt.callLater(() => input.forceActiveFocus());
            // The first time: the name without its extension, ready to type over.
            if (root.pane.namingFresh) {
                root.pane.namingFresh = false;
                select(0, root.pane.naming?.what === "rename" ? F.stemLength(text, root.dir) : text.length);
            } else cursorPosition = text.length;
        }
        onTextChanged: { root.pane.namingText = text; root.pane.namingError = "" }
        Keys.onReturnPressed: e => { e.accepted = true; const t = text; Qt.callLater(() => root.pane.named(t)) }
        Keys.onEnterPressed: e => { e.accepted = true; const t = text; Qt.callLater(() => root.pane.named(t)) }
        Keys.onEscapePressed: e => { e.accepted = true; root.pane.stopNaming() }
    }
    MouseArea { id: hover; anchors.fill: parent; hoverEnabled: true; acceptedButtons: Qt.NoButton }
    ToolTip.visible: root.wrong
    ToolTip.text: root.pane.namingError
}
