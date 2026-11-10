// Files › Shortcuts: every shortcut Files has. Click one and press the new
// keys (Esc leaves it, Backspace puts back the default). Files follows at
// once (files.keys holds only the changed ones).
import QtQuick
import qs
import qs.settings
import "FilesKeys.js" as FilesKeys

Column {
    id: page
    spacing: 16
    readonly property var overrides: Daemon.setting("files.keys", ({}))
    property string listening: ""
    property string clash: ""
    function keysFor(item) { return page.overrides[item.id] ?? item.keys }
    function save(item, keys) {
        const o = Object.assign({}, page.overrides);
        if (keys === "" || keys === item.keys) delete o[item.id]; else o[item.id] = keys;
        Daemon.set("files.keys", o);
    }
    // "Ctrl+Shift+K" from a key press (null while only modifiers are down).
    function combo(e) {
        const names = { [Qt.Key_Space]: "Space", [Qt.Key_Tab]: "Tab", [Qt.Key_Backtab]: "Tab", [Qt.Key_Return]: "Return", [Qt.Key_Enter]: "Enter",
                        [Qt.Key_Delete]: "Del", [Qt.Key_Backspace]: "Backspace", [Qt.Key_Left]: "Left", [Qt.Key_Right]: "Right",
                        [Qt.Key_Up]: "Up", [Qt.Key_Down]: "Down", [Qt.Key_PageUp]: "PgUp", [Qt.Key_PageDown]: "PgDown",
                        [Qt.Key_Home]: "Home", [Qt.Key_End]: "End", [Qt.Key_Insert]: "Ins", [Qt.Key_Equal]: "=", [Qt.Key_Minus]: "-" };
        if ([Qt.Key_Control, Qt.Key_Shift, Qt.Key_Alt, Qt.Key_Meta, Qt.Key_Super_L, Qt.Key_Super_R].includes(e.key)) return null;
        let k = names[e.key];
        if (!k && e.key >= Qt.Key_F1 && e.key <= Qt.Key_F35) k = "F" + (e.key - Qt.Key_F1 + 1);
        if (!k && e.key >= 0x21 && e.key < 0x7f) k = String.fromCharCode(e.key).toUpperCase();
        if (!k) return null;
        const mods = [];
        if (e.modifiers & Qt.ControlModifier) mods.push("Ctrl");
        if (e.modifiers & Qt.AltModifier) mods.push("Alt");
        if (e.modifiers & Qt.ShiftModifier || e.key === Qt.Key_Backtab) mods.push("Shift");
        if (e.modifiers & Qt.MetaModifier) mods.push("Meta");
        return mods.concat([k]).join("+");
    }

    PageHeader {
        title: "Shortcuts"
        subtitle: page.clash !== "" ? page.clash : page.listening !== "" ? "Press the new keys · Esc leaves it · Backspace: the default" : "Files › click one to change it."
        Button {
            text: "Reset all"
            enabled: Object.keys(page.overrides).length > 0
            onClicked: Daemon.set("files.keys", {})
        }
    }

    FocusScope {
        id: catcher
        width: parent.width
        height: list.height
        Keys.onPressed: e => {
            if (page.listening === "") return;
            e.accepted = true;
            const item = FilesKeys.list.find(i => i.id === page.listening);
            if (e.key === Qt.Key_Escape && e.modifiers === Qt.NoModifier) { page.listening = ""; return }
            if (e.key === Qt.Key_Backspace && e.modifiers === Qt.NoModifier) { page.save(item, ""); page.listening = ""; return }
            const c = page.combo(e);
            if (!c) return;
            const other = FilesKeys.list.find(i => i.id !== item.id && page.keysFor(i) === c);
            if (other) { page.clash = c + " is already " + other.label; return }
            page.clash = "";
            page.save(item, c);
            page.listening = "";
        }
        Card {
            id: list
            width: parent.width
            Repeater {
                model: FilesKeys.list
                delegate: SettingRow {
                    id: row
                    required property var modelData
                    readonly property bool on: page.listening === modelData.id
                    title: modelData.label
                    KeyCap {
                        text: row.on ? "…" : page.keysFor(row.modelData)
                        opacity: page.overrides[row.modelData.id] !== undefined || row.on ? 1 : 0.75
                    }
                    MouseArea {
                        parent: row
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: { page.clash = ""; page.listening = row.modelData.id; catcher.forceActiveFocus() }
                    }
                }
            }
        }
    }
}
