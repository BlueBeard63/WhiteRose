// Island view: the note (Figma: "Island v2 — Note · tasks"; the Note
// module, SUPER+SHIFT+N). One text, kept by whiterosed (`note.text`) and saved as
// you type: its first line is the title, "[ ]" lines are tasks. With tasks
// it opens on the checklist (how many are done, a tick each; click one to
// tick it); typing or Enter writes, Esc goes back to the checklist, or
// closes. Without tasks it opens writing.
import QtQuick
import qs
import "Note.js" as Note

Item {
    id: root
    implicitWidth: 460
    implicitHeight: (root.writing ? editor.height : list.height) + 36

    property string draft: Daemon.setting("note.text", "")
    readonly property var note: Note.parse(root.draft)
    property bool writing: true
    function close() { ShellState.close() }

    function save() { saver.stop(); if (root.draft !== Daemon.setting("note.text", "")) Daemon.set("note.text", root.draft) }
    Timer { id: saver; interval: 400; onTriggered: root.save() }
    function edit(text) { root.draft = text; saver.restart() }
    function write() {
        // (Ticks made in the checklist: the box shows them too.)
        if (area.text !== root.draft) area.text = root.draft;
        root.writing = true;
        Qt.callLater(() => { area.forceActiveFocus(); area.cursorPosition = area.length });
    }
    Component.onCompleted: {
        root.writing = root.note.tasks === 0;
        if (root.writing) root.write(); else keys.forceActiveFocus();
    }
    Component.onDestruction: root.save()

    // ---- writing --------------------------------------------------------------------
    Item {
        id: editor
        visible: root.writing
        x: 24; y: 18
        width: root.implicitWidth - 48
        height: Math.min(Math.max(area.contentHeight, 66), 280)
        Text {
            visible: area.length === 0
            text: "Title, then your lines"
            color: Theme.faint
            font { family: Theme.font; pointSize: Theme.pt(14) }
        }
        Flickable {
            anchors.fill: parent
            contentHeight: area.contentHeight
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            TextEdit {
                id: area
                width: editor.width
                text: root.draft
                wrapMode: TextEdit.Wrap
                color: Theme.text
                selectionColor: Theme.rose
                selectedTextColor: Theme.bg
                cursorDelegate: Rectangle { width: 1.5; color: Theme.rose; visible: area.cursorVisible }
                font { family: Theme.font; pointSize: Theme.pt(14) }
                Accessible.role: Accessible.EditableText
                Accessible.name: "Note"
                onTextChanged: if (text !== root.draft) root.edit(text)
                Keys.onEscapePressed: { root.save(); if (root.note.tasks > 0) { root.writing = false; keys.forceActiveFocus() } else root.close() }
            }
        }
    }

    // ---- the checklist --------------------------------------------------------------
    Item {
        id: keys
        focus: !root.writing
        Keys.onEscapePressed: root.close()
        Keys.onReturnPressed: root.write()
        Keys.onEnterPressed: root.write()
        Keys.onPressed: e => { if (e.text.length > 0 && e.text >= " ") { root.write(); e.accepted = true } }
    }
    Item {
        id: list
        visible: !root.writing
        x: 24; y: 18
        width: root.implicitWidth - 48
        height: Math.max(progress.height, rows.height)
        Column {
            id: progress
            width: 150
            spacing: 4
            Text {
                text: (root.note.title || "Note").toUpperCase()
                width: parent.width
                elide: Text.ElideRight
                color: Theme.faint
                font { family: Theme.mono; pointSize: Theme.pt(10); weight: Font.Medium; letterSpacing: 1 }
            }
            Text {
                text: root.note.done + "/" + root.note.tasks
                color: Theme.text
                font { family: Theme.mono; pointSize: Theme.pt(36); weight: Font.Medium }
                Accessible.role: Accessible.StaticText
                Accessible.name: root.note.done + " of " + root.note.tasks + " done"
            }
            Item { width: 1; height: 6 }
            // A segment a task (one bar when there are many).
            Row {
                spacing: 6
                readonly property int n: Math.min(root.note.tasks, 8)
                Repeater {
                    model: parent.n
                    delegate: Rectangle {
                        required property int index
                        readonly property int many: root.note.tasks
                        width: (138 - 6 * (parent.n - 1)) / parent.n; height: 4; radius: 2
                        color: (many <= 8 ? index < root.note.done : index < Math.round(root.note.done / many * 8)) ? Theme.rose : Theme.surface2
                    }
                }
            }
        }
        Rectangle { x: 162; width: 1; height: list.height; color: Theme.surface2 }
        Column {
            id: rows
            x: 186
            width: list.width - 186
            spacing: 10
            topPadding: 2
            Repeater {
                model: root.note.lines.slice(0, 7)
                delegate: Item {
                    required property var modelData
                    width: rows.width
                    height: 22
                    Accessible.role: modelData.task ? Accessible.CheckBox : Accessible.StaticText
                    Accessible.name: modelData.text
                    Accessible.checkable: modelData.task
                    Accessible.checked: modelData.done
                    Accessible.onPressAction: if (modelData.task) root.edit(Note.toggle(root.draft, modelData.at))
                    Rectangle {
                        visible: modelData.task
                        anchors.verticalCenter: parent.verticalCenter
                        width: 16; height: 16; radius: 8
                        color: modelData.done ? Theme.rose : "transparent"
                        border { width: modelData.done ? 0 : 1.5; color: Theme.muted }
                        Icon { anchors.centerIn: parent; visible: modelData.done; name: "check"; size: 11; color: Theme.bg }
                    }
                    Text {
                        x: modelData.task ? 28 : 0
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width - x
                        elide: Text.ElideRight
                        text: modelData.text
                        font { family: Theme.font; pointSize: Theme.pt(13); strikeout: modelData.done }
                        color: modelData.done ? Theme.faint : modelData.task ? Theme.text : Theme.muted
                    }
                    MouseArea {
                        anchors.fill: parent
                        enabled: modelData.task
                        cursorShape: Qt.PointingHandCursor
                        onClicked: root.edit(Note.toggle(root.draft, modelData.at))
                    }
                }
            }
            Text {
                visible: root.note.lines.length > 7
                text: "+" + (root.note.lines.length - 7) + " more"
                color: Theme.faint
                font { family: Theme.font; pointSize: Theme.pt(11) }
            }
        }
        // Clicking beside the tasks writes.
        MouseArea { x: 0; width: 150; height: list.height; cursorShape: Qt.IBeamCursor; onClicked: root.write() }
    }
}
