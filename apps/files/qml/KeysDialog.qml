// Keyboard shortcuts: every one Files has, with its keys. Click a shortcut
// and press the new keys (Esc: leave it; Backspace: back to the default).
// Saved in files.keys (whiterosed), so every Files window follows.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files

Popup {
    id: root
    property var list: []
    property var overrides: ({})
    property string listening: ""
    property string clash: ""

    function keysFor(item) { return root.overrides[item.id] ?? item.keys }
    function save(id, keys, item) {
        const o = Object.assign({}, root.overrides);
        if (keys === "" || keys === item.keys) delete o[id]; else o[id] = keys;
        Desktop.set("files.keys", JSON.stringify(o));
    }
    // "Ctrl+Shift+K" from a key press (null while only modifiers are down).
    function combo(e) {
        const names = { [Qt.Key_Space]: "Space", [Qt.Key_Tab]: "Tab", [Qt.Key_Backtab]: "Tab", [Qt.Key_Return]: "Return", [Qt.Key_Enter]: "Enter",
                        [Qt.Key_Delete]: "Del", [Qt.Key_Backspace]: "Backspace", [Qt.Key_Left]: "Left", [Qt.Key_Right]: "Right",
                        [Qt.Key_Up]: "Up", [Qt.Key_Down]: "Down", [Qt.Key_PageUp]: "PgUp", [Qt.Key_PageDown]: "PgDown",
                        [Qt.Key_Home]: "Home", [Qt.Key_End]: "End", [Qt.Key_Insert]: "Ins" };
        if ([Qt.Key_Control, Qt.Key_Shift, Qt.Key_Alt, Qt.Key_Meta, Qt.Key_Super_L, Qt.Key_Super_R].includes(e.key)) return null;
        let k = names[e.key];
        if (!k && e.key >= Qt.Key_F1 && e.key <= Qt.Key_F35) k = "F" + (e.key - Qt.Key_F1 + 1);
        if (!k && e.key >= 0x20 && e.key < 0x7f) k = String.fromCharCode(e.key).toUpperCase();
        if (!k) return null;
        const mods = [];
        if (e.modifiers & Qt.ControlModifier) mods.push("Ctrl");
        if (e.modifiers & Qt.AltModifier) mods.push("Alt");
        if (e.modifiers & Qt.ShiftModifier && e.key !== Qt.Key_Backtab) mods.push("Shift");
        if (e.key === Qt.Key_Backtab) mods.push("Shift");
        if (e.modifiers & Qt.MetaModifier) mods.push("Meta");
        return mods.concat([k]).join("+");
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    width: 480
    height: Math.min(parent ? parent.height - 80 : 600, 640)
    modal: true
    focus: true
    padding: 0
    background: Rectangle { color: Theme.surface1; radius: 16; border { width: 1; color: Theme.surface2 } }
    Overlay.modal: Rectangle { color: Qt.rgba(0, 0, 0, 0.35) }
    onClosed: { root.listening = ""; root.clash = "" }

    contentItem: FocusScope {
        focus: true
        Keys.onPressed: e => {
            e.accepted = true;
            if (root.listening === "") { if (e.key === Qt.Key_Escape) root.close(); return }
            const item = root.list.find(i => i.id === root.listening);
            if (e.key === Qt.Key_Escape && e.modifiers === Qt.NoModifier) { root.listening = ""; return }
            if (e.key === Qt.Key_Backspace && e.modifiers === Qt.NoModifier) { root.save(item.id, "", item); root.listening = ""; return }
            const c = root.combo(e);
            if (!c) return;
            const other = root.list.find(i => i.id !== item.id && root.keysFor(i) === c);
            if (other) { root.clash = c + " is " + other.label; return }
            root.clash = "";
            root.save(item.id, c, item);
            root.listening = "";
        }

        Text {
            id: head
            x: 22; y: 18
            text: "Keyboard shortcuts"
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
        }
        Text {
            anchors { left: head.left; top: head.bottom; topMargin: 4; right: parent.right; rightMargin: 22 }
            text: root.clash !== "" ? root.clash + " already" : root.listening !== "" ? "Press the new keys · Esc: leave it · Backspace: the default" : "Click one to change it"
            color: root.clash !== "" ? Theme.rose : Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(11.5) }
        }
        ListView {
            id: rows
            anchors { left: parent.left; right: parent.right; top: head.bottom; topMargin: 30; bottom: foot.top; bottomMargin: 6 }
            clip: true
            model: root.list
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
            delegate: Rectangle {
                required property var modelData
                readonly property bool on: root.listening === modelData.id
                readonly property bool changed: root.overrides[modelData.id] !== undefined
                x: 12; width: rows.width - 24; height: 36; radius: 9
                color: on ? Theme.tint(0.2) : rowArea.containsMouse ? Theme.surface2 : "transparent"
                Text {
                    anchors { left: parent.left; leftMargin: 10; verticalCenter: parent.verticalCenter }
                    text: modelData.label
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(12.5) }
                }
                Text {
                    anchors { right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
                    text: parent.on ? "…" : root.keysFor(modelData)
                    color: parent.changed ? Theme.rose : Theme.muted
                    font { family: Theme.mono; pointSize: Theme.pt(11.5) }
                }
                MouseArea { id: rowArea; anchors.fill: parent; hoverEnabled: true; onClicked: { root.clash = ""; root.listening = modelData.id } }
            }
        }
        Row {
            id: foot
            anchors { right: parent.right; rightMargin: 18; bottom: parent.bottom; bottomMargin: 16 }
            spacing: 8
            Button {
                text: "Reset all"
                kind: "ghost"
                enabled: Object.keys(root.overrides).length > 0
                onClicked: Desktop.set("files.keys", "{}")
            }
            Button { text: "Done"; kind: "primary"; onClicked: root.close() }
        }
    }
}
