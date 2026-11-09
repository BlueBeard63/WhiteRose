// WhiteRose Notepad: tabs (the scratch note first, then files), a menu,
// find and replace, and a status bar. Your open files come back next time.
//
// Ctrl+N new, Ctrl+O open, Ctrl+S save, Ctrl+Shift+S save as, Ctrl+W close
// tab, Ctrl+Tab / Ctrl+Shift+Tab switch, Ctrl+F find, Ctrl+H replace,
// Ctrl+G go to line, Ctrl+= / Ctrl+- / Ctrl+0 text size.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import WhiteRose.Kit
import WhiteRose.Notepad

AppWindow {
    id: win
    title: current ? current.title + (current.dirty ? " •" : "") + " — Notepad" : "Notepad"
    visible: true
    width: 960; height: 680

    property real zoom: 1
    // Line numbers on the left (Settings-free: the menu toggles it).
    property bool lineNumbers: true
    property var recent: []
    property int currentIndex: 0
    readonly property var editors: stack.children.filter(c => c.objectName === "editor")
    readonly property var current: editors[currentIndex] ?? null

    // ---- documents ----------------------------------------------------------------
    Component { id: editorComponent; Editor { objectName: "editor"; anchors.fill: parent; visible: false } }

    function newEditor(path, scratch) {
        return editorComponent.createObject(stack, { path: path, scratch: !!scratch });
    }
    function openFile(path) {
        const at = editors.findIndex(e => e.path === path);
        if (at >= 0) { show(at); return }
        const e = newEditor(path);
        if (!e.load()) {
            const err = e.error;
            e.destroy();
            notice.show("Couldn't open " + path.substring(path.lastIndexOf("/") + 1) + ": " + err);
            return;
        }
        Qt.callLater(() => { show(editors.indexOf(e)); remember(path) });
    }
    function newFile() {
        const e = newEditor("");
        Qt.callLater(() => show(editors.indexOf(e)));
    }
    function show(i) {
        if (i < 0 || i >= editors.length) return;
        win.currentIndex = i;
        editors.forEach((e, k) => e.visible = k === i);
        editors[i].area.forceActiveFocus();
        saveSession();
    }
    function remember(path) {
        win.recent = [path].concat(recent.filter(p => p !== path)).slice(0, 15);
        saveSession();
    }

    // Close a tab: asks first if it has unsaved changes.
    function closeTab(i) {
        const e = editors[i];
        if (!e || e.scratch) return; // the scratch note always stays
        if (e.dirty) { confirm.ask(e, () => reallyClose(e)); return }
        reallyClose(e);
    }
    function reallyClose(e) {
        const i = editors.indexOf(e);
        e.destroy();
        Qt.callLater(() => show(Math.min(i, editors.length - 1)));
    }

    function save(e, as) {
        if (!e) return;
        if (e.scratch && !as) { Files.saveScratch(e.text); return }
        if (as || e.path === "") { saveDialog.target = e; saveDialog.open(); return }
        const err = e.save();
        if (err) notice.show("Couldn't save: " + err);
    }

    // ---- session ---------------------------------------------------------------------
    function saveSession() {
        if (!started) return;
        const tabs = editors.slice(1).filter(e => e.path !== "").map(e => ({ path: e.path, cursor: e.area.cursorPosition }));
        Files.saveSession(JSON.stringify({ tabs: tabs, current: currentIndex, recent: recent }));
    }
    property bool started: false
    Component.onCompleted: {
        // The scratch note is always the first tab.
        const s = newEditor("", true);
        s.load();
        const session = JSON.parse(Files.session());
        win.recent = session.recent ?? [];
        for (const t of session.tabs ?? []) {
            const e = newEditor(t.path);
            if (e.load()) e.area.cursorPosition = Math.min(t.cursor ?? 0, e.area.length);
            else e.destroy();
        }
        win.started = true;
        Qt.callLater(() => {
            show(Math.min(session.current ?? 0, editors.length - 1));
            // Files given on the command line open last, in front.
            for (const p of JSON.parse(Files.arguments())) openFile(p);
        });
    }
    // Asking before closing with unsaved changes.
    onClosing: close => {
        saveSession();
        const dirty = editors.filter(e => e.dirty);
        if (dirty.length > 0 && !quitting) {
            close.accepted = false;
            confirm.ask(dirty[0], () => { win.quitting = true; win.close() }, dirty.length);
        }
    }
    property bool quitting: false

    // ---- title bar: tabs and the menu ---------------------------------------------------
    toolbar: [
        IconButton {
            anchors.verticalCenter: parent.verticalCenter
            width: 30; height: 30; icon: "menu"; iconSize: 15
            fill: "transparent"
            onClicked: mainMenu.openAt(0, height + 4)
            Menu {
                id: mainMenu
                items: [
                    { label: "New", shortcut: "Ctrl+N", run: () => win.newFile() },
                    { label: "Open…", shortcut: "Ctrl+O", run: () => openDialog.open() },
                    { label: "Save", shortcut: "Ctrl+S", enabled: !!win.current, run: () => win.save(win.current, false) },
                    { label: "Save as…", shortcut: "Ctrl+Shift+S", enabled: !!win.current, run: () => win.save(win.current, true) },
                    { separator: true },
                    { label: "Find", shortcut: "Ctrl+F", run: () => find.open(false) },
                    { label: "Replace", shortcut: "Ctrl+H", run: () => find.open(true) },
                    { label: "Go to line…", shortcut: "Ctrl+G", run: () => lineJump.open() },
                    { label: win.current?.wrap ? "Don't wrap lines" : "Wrap lines", run: () => { if (win.current) win.current.wrap = !win.current.wrap } },
                    { label: win.lineNumbers ? "Hide line numbers" : "Show line numbers", run: () => win.lineNumbers = !win.lineNumbers },
                    { separator: true },
                ].concat(win.recent.length === 0 ? [{ label: "No recent files", enabled: false, run: () => {} }]
                    : win.recent.slice(0, 8).map(p => ({ label: p.replace(/^\/home\/[^/]+/, "~"), run: () => win.openFile(p) })))
            }
        },
        Row {
            anchors.verticalCenter: parent.verticalCenter
            spacing: 4
            Repeater {
                model: win.editors
                delegate: Rectangle {
                    required property var modelData
                    required property int index
                    readonly property bool on: index === win.currentIndex
                    height: 30
                    width: Math.min(200, tabRow.implicitWidth + 24)
                    radius: 9
                    color: on ? Theme.surface1 : tabMouse.containsMouse ? Theme.surface1 : "transparent"
                    border { width: on ? 1 : 0; color: Theme.surface2 }
                    MouseArea {
                        id: tabMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        acceptedButtons: Qt.LeftButton | Qt.MiddleButton
                        onClicked: m => m.button === Qt.MiddleButton ? win.closeTab(index) : win.show(index)
                    }
                    Row {
                        id: tabRow
                        anchors { left: parent.left; leftMargin: 12; verticalCenter: parent.verticalCenter }
                        spacing: 6
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: modelData.title
                            width: Math.min(implicitWidth, 140)
                            elide: Text.ElideMiddle
                            color: parent.parent.on ? Theme.text : Theme.muted
                            font { family: Theme.font; pointSize: Theme.pt(12); weight: parent.parent.on ? Font.Medium : Font.Normal }
                        }
                        // Unsaved: a dot; hover: close.
                        Item {
                            visible: !modelData.scratch
                            width: 14; height: 14
                            anchors.verticalCenter: parent.verticalCenter
                            Rectangle { anchors.centerIn: parent; visible: modelData.dirty && !closeMouse.containsMouse; width: 7; height: 7; radius: 4; color: Theme.rose }
                            Icon { anchors.centerIn: parent; visible: !modelData.dirty || closeMouse.containsMouse; name: "x"; size: 11; color: closeMouse.containsMouse ? Theme.text : Theme.faint }
                            MouseArea { id: closeMouse; anchors.fill: parent; anchors.margins: -3; hoverEnabled: true; onClicked: win.closeTab(index) }
                        }
                    }
                }
            }
            IconButton { anchors.verticalCenter: parent.verticalCenter; width: 28; height: 28; icon: "plus"; iconSize: 13; fill: "transparent"; onClicked: win.newFile() }
        }
    ]

    // ---- the document, find bar and status bar ------------------------------------------
    // Changed on disk while you had edits.
    Rectangle {
        id: diskBanner
        visible: win.current?.changedOnDisk ?? false
        anchors { left: parent.left; right: parent.right; top: parent.top }
        height: visible ? 40 : 0
        color: Theme.tint(0.12)
        Text { anchors { left: parent.left; leftMargin: 16; verticalCenter: parent.verticalCenter } text: "This file changed on disk"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(12) } }
        Row {
            anchors { right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter }
            spacing: 8
            Button { text: "Reload"; onClicked: win.current.load() }
            Button { text: "Keep mine"; onClicked: { win.current.savedMtime = Files.mtime(win.current.path); win.current.changedOnDisk = false } }
        }
    }
    Item {
        id: stack
        anchors { left: parent.left; right: parent.right; top: diskBanner.bottom; bottom: find.top }
    }
    FindBar { id: find; editor: win.current; anchors { left: parent.left; right: parent.right; bottom: status.top } }
    Rectangle {
        id: status
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
        height: 28
        color: Theme.bg
        Rectangle { width: parent.width; height: 1; color: Theme.surface1 }
        readonly property var pos: {
            const e = win.current;
            if (!e) return { line: 1, col: 1 };
            const before = e.text.substring(0, e.area.cursorPosition);
            const nl = before.lastIndexOf("\n");
            return { line: before.split("\n").length, col: before.length - nl };
        }
        Text {
            anchors { left: parent.left; leftMargin: 16; verticalCenter: parent.verticalCenter }
            text: win.current?.scratch ? "Scratch note · saved as you type" : (win.current?.path || "Not saved yet")
            elide: Text.ElideMiddle
            width: parent.width / 2
            color: Theme.faint
            font { family: Theme.font; pointSize: Theme.pt(11) }
        }
        Row {
            anchors { right: parent.right; rightMargin: 16; verticalCenter: parent.verticalCenter }
            spacing: 16
            Repeater {
                model: ["Ln " + status.pos.line + ", Col " + status.pos.col, win.current?.language ?? "", (win.current?.eol ?? "lf").toUpperCase(), "UTF-8",
                        Math.round(win.zoom * 100) + "%"]
                delegate: Text { required property string modelData; text: modelData; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11) } }
            }
        }
    }

    // A message at the bottom for a few seconds (couldn't open, couldn't save).
    Rectangle {
        id: notice
        function show(t) { label.text = t; notice.visible = true; hide.restart() }
        visible: false
        anchors { horizontalCenter: parent.horizontalCenter; bottom: status.top; bottomMargin: 16 }
        width: label.implicitWidth + 32; height: 36; radius: 12
        color: Theme.surface2
        Text { id: label; anchors.centerIn: parent; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(12) } }
        Timer { id: hide; interval: 4000; onTriggered: notice.visible = false }
    }

    // ---- dialogs --------------------------------------------------------------------------
    FileDialog {
        id: openDialog
        title: "Open"
        fileMode: FileDialog.OpenFiles
        onAccepted: { for (const f of selectedFiles) win.openFile(decodeURIComponent(String(f).replace(/^file:\/\//, ""))) }
    }
    FileDialog {
        id: saveDialog
        property var target: null
        title: "Save as"
        fileMode: FileDialog.SaveFile
        onAccepted: {
            const p = decodeURIComponent(String(selectedFile).replace(/^file:\/\//, ""));
            const t = target;
            // Saving the scratch note "as" a file makes a new tab for it.
            if (t.scratch) {
                const r = JSON.parse(Files.save(p, t.text, "lf"));
                if (!r.ok) { notice.show("Couldn't save: " + r.error); return }
                win.openFile(p);
                return;
            }
            const err = t.save(p);
            if (err) notice.show("Couldn't save: " + err);
            else win.remember(p);
        }
    }

    // Unsaved changes: Save, Don't save, Cancel.
    Popup {
        id: confirm
        property var editor: null
        property var then: null
        property int count: 1
        function ask(e, done, n) { confirm.editor = e; confirm.then = done; confirm.count = n ?? 1; confirm.open() }
        anchors.centerIn: Overlay.overlay
        modal: true
        padding: 22
        width: 400
        background: Rectangle { color: Theme.bg; radius: 16; border { width: 1; color: Theme.surface2 } }
        Overlay.modal: Rectangle { color: Qt.rgba(0, 0, 0, 0.45) }
        contentItem: Column {
            spacing: 16
            Text {
                width: parent.width
                wrapMode: Text.Wrap
                text: confirm.count > 1 ? confirm.count + " files have unsaved changes" : "Save changes to " + (confirm.editor?.title ?? "") + "?"
                color: Theme.text
                font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
            }
            Row {
                anchors.right: parent.right
                spacing: 8
                Button { text: "Cancel"; onClicked: confirm.close() }
                Button { text: "Don't save"; kind: "danger"; onClicked: { confirm.close(); win.editors.forEach(e => { if (confirm.count > 1 || e === confirm.editor) e.dirty = false }); confirm.then() } }
                Button {
                    text: "Save"; kind: "primary"
                    onClicked: {
                        confirm.close();
                        const todo = confirm.count > 1 ? win.editors.filter(e => e.dirty) : [confirm.editor];
                        for (const e of todo) {
                            if (e.path === "") { win.save(e, true); return }
                            const err = e.save();
                            if (err) { notice.show("Couldn't save: " + err); return }
                        }
                        confirm.then();
                    }
                }
            }
        }
    }

    Popup {
        id: lineJump
        anchors.centerIn: Overlay.overlay
        padding: 16
        background: Rectangle { color: Theme.surface1; radius: 14; border { width: 1; color: Theme.surface2 } }
        onOpened: { lineField.text = ""; lineField.focusField() }
        contentItem: Field {
            id: lineField
            implicitWidth: 220
            placeholder: "Go to line"
            // Closed once Enter is handled (closing during it can crash Qt).
            onAccepted: { const n = parseInt(text); Qt.callLater(() => { lineJump.close(); if (!isNaN(n)) win.current?.goToLine(n) }) }
        }
    }

    // ---- keys --------------------------------------------------------------------------------
    Shortcut { sequence: "Ctrl+N"; onActivated: win.newFile() }
    Shortcut { sequence: "Ctrl+O"; onActivated: openDialog.open() }
    Shortcut { sequence: "Ctrl+S"; onActivated: win.save(win.current, false) }
    Shortcut { sequence: "Ctrl+Shift+S"; onActivated: win.save(win.current, true) }
    Shortcut { sequence: "Ctrl+W"; onActivated: win.closeTab(win.currentIndex) }
    Shortcut { sequence: "Ctrl+Tab"; onActivated: win.show((win.currentIndex + 1) % win.editors.length) }
    Shortcut { sequence: "Ctrl+Shift+Tab"; onActivated: win.show((win.currentIndex - 1 + win.editors.length) % win.editors.length) }
    Shortcut { sequence: "Ctrl+F"; onActivated: find.open(false) }
    Shortcut { sequence: "Ctrl+H"; onActivated: find.open(true) }
    Shortcut { sequence: "Ctrl+G"; onActivated: lineJump.open() }
    Shortcut { sequences: ["Ctrl+=", "Ctrl++"]; onActivated: win.zoom = Math.min(3, win.zoom + 0.1) }
    Shortcut { sequence: "Ctrl+-"; onActivated: win.zoom = Math.max(0.6, win.zoom - 0.1) }
    Shortcut { sequence: "Ctrl+0"; onActivated: win.zoom = 1 }
}
