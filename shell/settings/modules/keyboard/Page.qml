// Keyboard & Shortcuts (Figma: "Settings - Keyboard & Shortcuts"): layouts
// (SUPER+SPACE switches), key repeat, and every shortcut, each bound to an
// action so anything can be remapped. Your own shortcuts run a command.
//
// Recording a shortcut: the compositor pauses its shortcuts so the keys
// reach this page. A combo another shortcut uses is shown with a warning;
// Enter saves it (taking it from the other), Esc cancels.
import QtQuick
import QtQuick.Controls
import Quickshell
import Quickshell.Io
import qs
import qs.settings
import "Keys.js" as K

Column {
    id: page
    spacing: 16

    readonly property var shortcuts: Compositor.shortcuts
    readonly property var overrides: Daemon.setting("shortcuts.keys", ({}))
    readonly property var custom: Daemon.setting("shortcuts.custom", [])
    readonly property var state: ({ shortcuts: page.shortcuts, overrides: page.overrides, custom: page.custom })

    function save(next) {
        if (JSON.stringify(next.overrides) !== JSON.stringify(page.overrides)) Daemon.set("shortcuts.keys", next.overrides);
        if (JSON.stringify(next.custom) !== JSON.stringify(page.custom)) Daemon.set("shortcuts.custom", next.custom);
    }
    function keysOf(id) { return (page.shortcuts.find(s => s.id === id) ?? { keys: [] }).keys }

    // ---- recording --------------------------------------------------------------
    property string recording: ""      // shortcut id being recorded
    property bool recordAdd: false      // add to its keys instead of replacing
    property string held: ""           // what's held so far ("SUPER+SHIFT")
    property string captured: ""       // the finished combo, waiting for Enter (conflict)
    property string recordError: ""
    readonly property var clash: page.captured ? K.conflict(page.shortcuts, page.captured, page.recording) : null

    function record(id, add) {
        page.recording = id;
        page.recordAdd = add;
        page.held = "";
        page.captured = "";
        page.recordError = "";
        Compositor.pauseShortcuts(true);
        catcher.forceActiveFocus();
    }
    function stopRecording() {
        if (page.recording === "") return;
        page.recording = "";
        page.held = "";
        page.captured = "";
        page.recordError = "";
        Compositor.pauseShortcuts(false);
    }
    function finish(c) {
        const range = page.shortcuts.find(s => s.id === page.recording)?.keys.some(k => k.endsWith("1-9"))
            || ["workspace", "move-to-workspace"].includes(page.recording);
        if (range) {
            const r = K.forRange(c);
            if (!r) { page.recordError = "Use a number key: it stands for 1 to 9"; page.held = ""; return; }
            c = r;
        }
        page.captured = K.normalize(c);
        if (!page.clash) commit();
    }
    function commit() {
        page.save(K.assign(page.state, page.recording, page.captured, page.recordAdd));
        stopRecording();
    }
    Component.onDestruction: if (recording !== "") Compositor.pauseShortcuts(false)

    // Takes the keys while recording.
    Item {
        id: catcher
        width: 0; height: 0
        property bool superAlone: false
        function mods(e) {
            return { logo: (e.modifiers & Qt.MetaModifier) !== 0, ctrl: (e.modifiers & Qt.ControlModifier) !== 0,
                     alt: (e.modifiers & Qt.AltModifier) !== 0, shift: (e.modifiers & Qt.ShiftModifier) !== 0 };
        }
        Keys.onPressed: e => {
            e.accepted = true;
            if (page.recording === "") return;
            if (page.captured !== "") {
                if (e.key === Qt.Key_Return || e.key === Qt.Key_Enter) page.commit();
                else if (e.key === Qt.Key_Escape) page.stopRecording();
                return;
            }
            const m = mods(e);
            if (K.isModifier(e.key)) {
                if (K.isSuper(e.key)) m.logo = true;
                superAlone = K.isSuper(e.key) && !m.ctrl && !m.alt && !m.shift;
                page.held = K.combo(m, "");
                return;
            }
            superAlone = false;
            if (e.key === Qt.Key_Escape && !m.logo && !m.ctrl && !m.alt && !m.shift) { page.stopRecording(); return; }
            const name = K.keyName(e.key, e.nativeScanCode);
            if (name === "") { page.recordError = "That key can't be a shortcut"; return; }
            page.recordError = "";
            page.finish(K.combo(m, name));
        }
        Keys.onReleased: e => {
            e.accepted = true;
            if (page.recording === "" || page.captured !== "") return;
            // SUPER pressed and released on its own: a tap (like the launcher's).
            if (K.isSuper(e.key) && superAlone) { superAlone = false; page.finish("SUPER"); return; }
            if (K.isModifier(e.key)) page.held = K.combo(mods(e), "");
        }
        onActiveFocusChanged: if (!activeFocus) page.stopRecording()
    }

    // ---- header -----------------------------------------------------------------
    PageHeader {
        title: "Keyboard & Shortcuts"
        subtitle: "Shortcuts are bound to actions, so anything can be remapped."
        Button { text: "Add shortcut"; icon: "plus"; onClicked: customDialog.openFor(null) }
    }

    // ---- layouts and repeat ---------------------------------------------------------
    readonly property var layouts: Daemon.setting("keyboard.layouts", [])
    // In use: the ones set here, else the system's (named by the compositor).
    readonly property var inUse: Compositor.keyboardLayouts.length > 0 ? Compositor.keyboardLayouts
        : page.layouts.map(l => Object.assign({ name: l.layout }, l))
    function setLayouts(list) { Daemon.set("keyboard.layouts", list.map(l => l.variant ? { layout: l.layout, variant: l.variant } : { layout: l.layout })) }
    readonly property string switchKeys: (page.shortcuts.find(s => s.id === "switch-layout")?.keys ?? [])[0] ?? ""

    Card {
        SettingRow {
            title: "Layouts"
            subtitle: page.inUse.length > 1 && page.switchKeys ? K.spoken(page.switchKeys) + " to switch" : "The first is the default"
            Repeater {
                model: page.inUse
                delegate: Rectangle {
                    required property var modelData
                    required property int index
                    readonly property bool first: index === 0
                    width: chipRow.implicitWidth + 12; height: 28; radius: 7
                    anchors.verticalCenter: parent.verticalCenter
                    color: chipMouse.containsMouse ? Theme.surface2 : "transparent"
                    Row {
                        id: chipRow
                        anchors.centerIn: parent
                        spacing: 6
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: modelData.name + (index < page.inUse.length - 1 ? "," : "")
                            color: parent.parent.first ? Theme.text : Theme.muted
                            font { family: Theme.font; pointSize: Theme.pt(12); weight: parent.parent.first ? Font.Medium : Font.Normal }
                        }
                        // Remove (not the last one).
                        Icon {
                            visible: page.inUse.length > 1 && chipMouse.containsMouse
                            anchors.verticalCenter: parent.verticalCenter
                            name: "x"; size: 11; color: Theme.muted
                        }
                    }
                    MouseArea {
                        id: chipMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        // Click: make it the default. Click the ×: remove it.
                        onClicked: m => {
                            const list = page.inUse.slice();
                            if (page.inUse.length > 1 && m.x > width - 22) list.splice(index, 1);
                            else if (index > 0) list.unshift(list.splice(index, 1)[0]);
                            else return;
                            page.setLayouts(list);
                        }
                    }
                }
            }
            Rectangle {
                width: 28; height: 28; radius: 8
                anchors.verticalCenter: parent.verticalCenter
                color: addMouse.containsMouse ? Theme.grip : Theme.surface2
                Icon { anchors.centerIn: parent; name: "plus"; size: 13; color: Theme.muted }
                MouseArea { id: addMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: layoutDialog.open() }
            }
        }
        SettingRow {
            id: delayRow
            title: "Repeat delay"
            readonly property int value: Daemon.setting("keyboard.repeatDelay", 300)
            property int shown: value
            onValueChanged: shown = value
            Slider {
                implicitWidth: 200
                value: delayRow.shown - 150; maximum: 850
                onMoved: v => { delayRow.shown = Math.round((v + 150) / 10) * 10; delaySave.restart() }
            }
            Text { width: 64; horizontalAlignment: Text.AlignRight; anchors.verticalCenter: parent.verticalCenter; text: delayRow.shown + " ms"; color: Theme.muted; font { family: Theme.mono; pointSize: Theme.pt(11.5) } }
            Timer { id: delaySave; interval: 250; onTriggered: Daemon.set("keyboard.repeatDelay", delayRow.shown) }
        }
        SettingRow {
            id: rateRow
            title: "Repeat rate"
            readonly property int value: Daemon.setting("keyboard.repeatRate", 30)
            property int shown: value
            onValueChanged: shown = value
            Slider {
                implicitWidth: 200
                value: rateRow.shown - 5; maximum: 75
                onMoved: v => { rateRow.shown = Math.round(v + 5); rateSave.restart() }
            }
            Text { width: 64; horizontalAlignment: Text.AlignRight; anchors.verticalCenter: parent.verticalCenter; text: rateRow.shown + " / s"; color: Theme.muted; font { family: Theme.mono; pointSize: Theme.pt(11.5) } }
            Timer { id: rateSave; interval: 250; onTriggered: Daemon.set("keyboard.repeatRate", rateRow.shown) }
        }
    }

    // ---- search and groups ---------------------------------------------------------------
    property string query: ""
    property string group: "All"
    readonly property var groups: ["All", "Shell", "Windows", "Workspaces", "Media"].concat(page.custom.length > 0 ? ["Custom"] : [])
    readonly property var shown: page.shortcuts.filter(s =>
        (page.group === "All" || s.group === page.group)
        && (page.query === "" || [s.name, s.group, s.command ?? "", ...s.keys.map(K.spoken)].some(t => t.toLowerCase().includes(page.query.toLowerCase()))))

    Row {
        width: parent.width
        spacing: 8
        Rectangle {
            width: 250; height: 34; radius: 10
            color: Theme.surface1
            Icon { x: 11; anchors.verticalCenter: parent.verticalCenter; name: "search"; size: 14; color: Theme.muted }
            TextInput {
                id: search
                anchors { left: parent.left; leftMargin: 32; right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
                color: Theme.text
                selectionColor: Theme.rose
                clip: true
                font { family: Theme.font; pointSize: Theme.pt(12.5) }
                onTextChanged: page.query = text.trim()
                Keys.onEscapePressed: text = ""
                Text {
                    visible: search.text === ""
                    anchors.verticalCenter: parent.verticalCenter
                    text: "Search shortcuts"
                    color: Theme.faint
                    font: search.font
                }
            }
        }
        Segmented {
            options: page.groups.map(g => ({ label: g, value: g }))
            value: page.group
            onPicked: v => page.group = v
        }
    }

    // ---- the shortcuts ----------------------------------------------------------------------
    Card {
        SettingRow {
            visible: page.shown.length === 0
            divider: false
            title: page.shortcuts.length === 0 ? "Waiting for the compositor" : "No shortcuts match"
        }
        Repeater {
            model: page.shown
            delegate: Rectangle {
                id: row
                required property var modelData
                required property int index
                readonly property bool rec: page.recording === modelData.id
                readonly property bool changed: !modelData.command && JSON.stringify(modelData.keys) !== JSON.stringify(modelData.defaults)
                width: parent.width
                height: 54
                color: rec ? Theme.tint(0.08) : "transparent"
                Rectangle { visible: row.index > 0; width: parent.width; height: 1; color: Theme.surface2 }

                Column {
                    anchors { left: parent.left; leftMargin: 16; right: keysArea.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
                    spacing: 2
                    Text {
                        width: parent.width; elide: Text.ElideRight
                        text: row.modelData.name
                        color: Theme.text
                        font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.Medium }
                    }
                    Text {
                        width: parent.width; elide: Text.ElideRight
                        readonly property bool warn: row.rec && (page.clash !== null || page.recordError !== "")
                        text: row.rec && page.recordError ? page.recordError
                            : row.rec && page.clash ? K.spoken(page.captured) + " is used by “" + page.clash.name + "” — saving will replace it"
                            : row.modelData.command ? row.modelData.command
                            : row.modelData.group + (row.changed ? " · changed" : "")
                        color: warn ? Theme.rose : Theme.muted
                        font { family: row.modelData.command && !warn ? Theme.mono : Theme.font; pointSize: Theme.pt(11) }
                    }
                }

                Row {
                    id: keysArea
                    anchors { right: more.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
                    spacing: 10

                    // Recording: what's held, in a dashed box.
                    Rectangle {
                        visible: row.rec
                        anchors.verticalCenter: parent.verticalCenter
                        width: recRow.implicitWidth + 20; height: 30; radius: 8
                        color: "transparent"
                        border { width: 1; color: Theme.pearl }
                        Row {
                            id: recRow
                            anchors.centerIn: parent
                            spacing: 5
                            Rectangle { width: 6; height: 6; radius: 3; color: Theme.rose; anchors.verticalCenter: parent.verticalCenter }
                            Repeater {
                                model: K.caps(page.captured || page.held)
                                delegate: KeyCap { required property string modelData; text: modelData }
                            }
                            Text {
                                visible: page.captured === ""
                                anchors.verticalCenter: parent.verticalCenter
                                text: "…"
                                color: Theme.muted
                                font { family: Theme.font; pointSize: Theme.pt(11) }
                            }
                        }
                    }
                    Text {
                        visible: row.rec
                        anchors.verticalCenter: parent.verticalCenter
                        text: page.captured ? "enter to save · esc to cancel" : "esc to cancel"
                        color: Theme.faint
                        font { family: Theme.font; pointSize: Theme.pt(10.5) }
                    }

                    // The keys, each combo a group of caps.
                    Repeater {
                        model: row.rec ? [] : row.modelData.keys
                        delegate: Row {
                            required property string modelData
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: 4
                            Repeater {
                                model: K.caps(modelData)
                                delegate: KeyCap { required property string modelData; text: modelData }
                            }
                        }
                    }
                    Text {
                        visible: !row.rec && row.modelData.keys.length === 0
                        anchors.verticalCenter: parent.verticalCenter
                        text: "Off"
                        color: Theme.faint
                        font { family: Theme.font; pointSize: Theme.pt(11.5) }
                    }
                }

                // Click the keys (or the row) to record new ones.
                MouseArea {
                    anchors { left: parent.left; right: more.left; top: parent.top; bottom: parent.bottom }
                    cursorShape: Qt.PointingHandCursor
                    onClicked: row.rec ? page.stopRecording() : page.record(row.modelData.id, false)
                }

                Rectangle {
                    id: more
                    anchors { right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
                    width: 28; height: 28; radius: 8
                    color: moreMouse.containsMouse || menu.opened ? Theme.surface2 : "transparent"
                    Row {
                        anchors.centerIn: parent
                        spacing: 3
                        Repeater { model: 3; delegate: Rectangle { width: 3; height: 3; radius: 1.5; color: Theme.muted } }
                    }
                    MouseArea { id: moreMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: menu.open() }

                    Popup {
                        id: menu
                        y: more.height + 4
                        x: more.width - width
                        width: Math.max(190, menuList.implicitWidth + 12)
                        padding: 6
                        background: Rectangle { color: Theme.surface1; radius: 12; border { width: 1; color: Theme.surface2 } }
                        readonly property var items: [
                            { label: "Change shortcut", run: () => page.record(row.modelData.id, false) },
                            { label: "Add another", show: row.modelData.keys.length > 0 && row.modelData.keys.length < 4 && !row.modelData.keys.some(k => k.endsWith("1-9")), run: () => page.record(row.modelData.id, true) },
                            { label: "Reset to default", show: row.changed, run: () => page.save(K.setKeys(page.state, row.modelData.id, row.modelData.defaults)) },
                            { label: "Turn off", show: row.modelData.keys.length > 0, run: () => page.save(K.setKeys(page.state, row.modelData.id, [])) },
                            { label: "Edit…", show: !!row.modelData.command, run: () => customDialog.openFor(page.custom.find(c => c.id === row.modelData.id)) },
                            { label: "Delete", show: !!row.modelData.command, danger: true, run: () => Daemon.set("shortcuts.custom", page.custom.filter(c => c.id !== row.modelData.id)) },
                        ].filter(i => i.show !== false)
                        contentItem: Column {
                            id: menuList
                            spacing: 2
                            Repeater {
                                model: menu.items
                                delegate: Rectangle {
                                    required property var modelData
                                    width: Math.max(menu.width - 12, itemText.implicitWidth + 24)
                                    height: 30; radius: 8
                                    color: itemMouse.containsMouse ? Theme.surface2 : "transparent"
                                    Text {
                                        id: itemText
                                        anchors { left: parent.left; leftMargin: 12; verticalCenter: parent.verticalCenter }
                                        text: modelData.label
                                        color: modelData.danger ? Theme.rose : Theme.text
                                        font { family: Theme.font; pointSize: Theme.pt(12) }
                                    }
                                    MouseArea {
                                        id: itemMouse
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: Qt.PointingHandCursor
                                        onClicked: { menu.close(); modelData.run() }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // ---- add or edit your own ------------------------------------------------------------------
    Dialog {
        id: customDialog
        property var editing: null
        function openFor(c) {
            editing = c;
            nameField.text = c?.name ?? "";
            commandField.text = c?.command ?? "";
            open();
            nameField.focusField();
        }
        readonly property bool valid: nameField.text.trim() !== "" && commandField.text.trim() !== ""
        function save() {
            if (!valid) return;
            const list = page.custom.map(c => Object.assign({}, c));
            let id;
            if (editing) {
                id = editing.id;
                const c = list.find(x => x.id === id);
                c.name = nameField.text.trim();
                c.command = commandField.text.trim();
            } else {
                id = K.newId(nameField.text, list);
                list.push({ id: id, name: nameField.text.trim(), command: commandField.text.trim(), keys: [] });
            }
            Daemon.set("shortcuts.custom", list);
            close();
            // A new one: record its keys straight away.
            if (!editing) { page.group = "All"; search.text = ""; recordNew.id = id; recordNew.start(); }
        }
        title: editing ? "Edit shortcut" : "Add shortcut"
        subtitle: "Runs a command"
        icon: "terminal"
        Text { text: "Name"; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
        Field { id: nameField; width: parent.width; placeholder: "Open Files"; onAccepted: commandField.focusField() }
        Text { text: "Command"; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
        Field { id: commandField; width: parent.width; mono: true; placeholder: "nautilus"; onAccepted: customDialog.save() }
        footer: [
            Button { id: cancelCustom; text: "Cancel"; onClicked: customDialog.close() },
            Button { id: saveCustom; text: customDialog.editing ? "Save" : "Add and set keys"; kind: "primary"; enabled: customDialog.valid; onClicked: customDialog.save() }
        ]
    }
    // Starts recording once the new shortcut has arrived from the compositor.
    Timer {
        id: recordNew
        property string id: ""
        interval: 100; repeat: true
        property int tries: 0
        onTriggered: {
            if (page.shortcuts.some(s => s.id === id) || ++tries > 30) { stop(); tries = 0; if (page.shortcuts.some(s => s.id === id)) page.record(id, false); }
        }
    }

    // ---- choosing a layout -----------------------------------------------------------------------
    property var layoutDb: ({ layouts: [], variants: [] })
    FileView {
        path: "/usr/share/X11/xkb/rules/evdev.lst"
        onLoaded: page.layoutDb = K.parseLayouts(text())
    }
    Dialog {
        id: layoutDialog
        title: "Add a layout"
        subtitle: "Search by language or country"
        icon: "plus"
        onOpened: { layoutSearch.text = ""; layoutSearch.focusField() }
        Field { id: layoutSearch; width: parent.width; placeholder: "Search layouts" }
        ListView {
            width: parent.width
            height: 300
            clip: true
            model: K.searchLayouts(page.layoutDb, layoutSearch.text).filter(l => !page.inUse.some(u => u.layout === l.layout && (u.variant ?? "") === l.variant))
            boundsBehavior: Flickable.StopAtBounds
            delegate: Rectangle {
                required property var modelData
                width: ListView.view.width
                height: 36; radius: 9
                color: pickMouse.containsMouse ? Theme.surface1 : "transparent"
                Text {
                    anchors { left: parent.left; leftMargin: 10; verticalCenter: parent.verticalCenter }
                    text: modelData.name
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(12.5) }
                }
                Text {
                    anchors { right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
                    text: modelData.layout + (modelData.variant ? " · " + modelData.variant : "")
                    color: Theme.faint
                    font { family: Theme.mono; pointSize: Theme.pt(10.5) }
                }
                MouseArea {
                    id: pickMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: { page.setLayouts(page.inUse.concat([modelData])); layoutDialog.close() }
                }
            }
        }
    }
}
