// WhiteRose Files: tabs, the sidebar, the address bar, and the current
// folder as a grid, a list or columns.
//
// Ctrl+T new tab, Ctrl+W close, Ctrl+Tab switch, Ctrl+L type a path,
// Alt+Left / Alt+Right back and forward, Alt+Up or Backspace up,
// Ctrl+1 / 2 / 3 grid, list, columns, Ctrl+H hidden files, Ctrl+A all,
// Ctrl+D bookmark this folder, F5 reload.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F

AppWindow {
    id: win
    title: current ? current.title + " — Files" : "Files"
    visible: true
    width: 1040; height: 680

    property int currentIndex: 0
    readonly property var panes: stack.children.filter(c => c.objectName === "pane")
    readonly property var current: panes[currentIndex] ?? null

    Component {
        id: paneComponent
        Pane {
            objectName: "pane"
            anchors.fill: parent
            visible: false
            onOpenFile: entry => Qt.openUrlExternally("file://" + entry.path)
        }
    }
    function newTab(path) {
        const p = paneComponent.createObject(stack, path ? { path: path } : {});
        // New tabs look like the one you're in.
        if (win.current) { p.mode = win.current.mode; p.hidden = win.current.hidden }
        Qt.callLater(() => win.show(win.panes.indexOf(p)));
    }
    function show(i) {
        if (i < 0 || i >= win.panes.length) return;
        win.currentIndex = i;
        win.panes.forEach((p, k) => p.visible = k === i);
        win.panes[i].focusView();
    }
    function closeTab(i) {
        if (win.panes.length <= 1) { win.close(); return }
        const p = win.panes[i];
        p.destroy();
        Qt.callLater(() => win.show(Math.min(i, win.panes.length - 1)));
    }
    Component.onCompleted: {
        const args = JSON.parse(Fs.arguments());
        if (args.length === 0) win.newTab("");
        for (const a of args) win.newTab(a);
    }

    // ---- title bar: tabs ---------------------------------------------------------------
    toolbar: [
        Row {
            anchors.verticalCenter: parent.verticalCenter
            spacing: 4
            Repeater {
                model: win.panes
                delegate: Rectangle {
                    required property var modelData
                    required property int index
                    readonly property bool on: index === win.currentIndex
                    height: 30
                    width: Math.min(200, tabRow.implicitWidth + 24)
                    radius: 9
                    color: on || tabArea.containsMouse ? Theme.surface1 : "transparent"
                    border { width: on ? 1 : 0; color: Theme.surface2 }
                    MouseArea {
                        id: tabArea
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
                        Item {
                            visible: win.panes.length > 1
                            width: 14; height: 14
                            anchors.verticalCenter: parent.verticalCenter
                            Icon { anchors.centerIn: parent; name: "x"; size: 11; color: closeArea.containsMouse ? Theme.text : Theme.faint }
                            MouseArea { id: closeArea; anchors.fill: parent; anchors.margins: -3; hoverEnabled: true; onClicked: win.closeTab(index) }
                        }
                    }
                }
            }
            IconButton { anchors.verticalCenter: parent.verticalCenter; width: 28; height: 28; icon: "plus"; iconSize: 13; fill: "transparent"; onClicked: win.newTab(win.current?.path ?? "") }
        }
    ]

    // ---- sidebar ------------------------------------------------------------------------
    Sidebar {
        id: sidebar
        anchors { left: parent.left; top: parent.top; bottom: parent.bottom }
        width: 210
        current: win.current?.path ?? ""
        onChosen: p => { win.current?.go(p); win.current?.focusView() }
    }

    // ---- navigation row --------------------------------------------------------------------
    Item {
        id: nav
        anchors { left: sidebar.right; right: parent.right; top: parent.top }
        height: 48
        Row {
            id: arrows
            anchors { left: parent.left; leftMargin: 12; verticalCenter: parent.verticalCenter }
            spacing: 2
            IconButton { width: 32; height: 32; icon: "chevL"; fill: "transparent"; opacity: (win.current?.back.length ?? 0) > 0 ? 1 : 0.35; onClicked: win.current?.goBack() }
            IconButton { width: 32; height: 32; icon: "chevR"; fill: "transparent"; opacity: (win.current?.forward.length ?? 0) > 0 ? 1 : 0.35; onClicked: win.current?.goForward() }
            IconButton { width: 32; height: 32; icon: "chevUp"; fill: "transparent"; opacity: win.current?.path !== "/" ? 1 : 0.35; onClicked: win.current?.up() }
        }
        AddressBar {
            id: address
            anchors { left: arrows.right; leftMargin: 8; right: views.left; rightMargin: 10; verticalCenter: parent.verticalCenter }
            path: win.current?.path ?? "/"
            hidden: win.current?.hidden ?? false
            onGo: p => { win.current?.go(p); win.current?.focusView() }
            onDone: win.current?.focusView()
            z: 5
        }
        Segmented {
            id: views
            anchors { right: menuButton.left; rightMargin: 6; verticalCenter: parent.verticalCenter }
            value: win.current?.mode ?? "grid"
            options: [{ label: "Grid", value: "grid" }, { label: "List", value: "list" }, { label: "Columns", value: "columns" }]
            onPicked: v => { if (win.current) { win.current.mode = v; Qt.callLater(() => win.current.focusView()) } }
        }
        IconButton {
            id: menuButton
            anchors { right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter }
            width: 32; height: 32; icon: "menu"; fill: "transparent"
            onClicked: viewMenu.popup(width - viewMenu.width, height + 4)
            Menu {
                id: viewMenu
                readonly property bool bookmarked: sidebar.places.bookmarks.some(b => b.path === win.current?.path)
                items: [
                    { label: "New tab", shortcut: "Ctrl+T", run: () => win.newTab(win.current?.path ?? "") },
                    { label: (win.current?.hidden ? "Hide" : "Show") + " hidden files", shortcut: "Ctrl+H", run: () => { if (win.current) win.current.hidden = !win.current.hidden } },
                    { label: viewMenu.bookmarked ? "Remove bookmark" : "Bookmark this folder", shortcut: "Ctrl+D", run: () => win.toggleBookmark() },
                    { separator: true },
                    { label: "Sort by name", run: () => { win.current.sortBy = "name"; win.current.descending = false } },
                    { label: "Sort by size", run: () => { win.current.sortBy = "size"; win.current.descending = true } },
                    { label: "Sort by date modified", run: () => { win.current.sortBy = "modified"; win.current.descending = true } },
                    { label: "Sort by kind", run: () => { win.current.sortBy = "kind"; win.current.descending = false } },
                ]
            }
        }
        Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.surface1 }
    }
    function toggleBookmark() {
        if (!win.current) return;
        const on = !sidebar.places.bookmarks.some(b => b.path === win.current.path);
        Fs.setBookmark(win.current.path, on);
        sidebar.refresh();
    }

    // ---- the folder -----------------------------------------------------------------------
    Item {
        id: stack
        anchors { left: sidebar.right; right: parent.right; top: nav.bottom; bottom: status.top }
    }

    // ---- status bar ------------------------------------------------------------------------
    Rectangle {
        id: status
        anchors { left: sidebar.right; right: parent.right; bottom: parent.bottom }
        height: 28
        color: Theme.bg
        Rectangle { width: parent.width; height: 1; color: Theme.surface1 }
        Text {
            anchors { left: parent.left; leftMargin: 16; verticalCenter: parent.verticalCenter }
            text: {
                const p = win.current;
                if (!p) return "";
                const n = p.shownEntries.length, sel = p.selection;
                const items = n + (n === 1 ? " item" : " items");
                if (sel.length === 0) return items;
                const bytes = sel.filter(e => !e.dir).reduce((a, e) => a + e.size, 0);
                return items + " · " + sel.length + " selected" + (bytes > 0 ? " (" + F.size(bytes) + ")" : "");
            }
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(11) }
        }
    }

    // ---- keys -------------------------------------------------------------------------------
    Shortcut { sequence: "Ctrl+T"; onActivated: win.newTab(win.current?.path ?? "") }
    Shortcut { sequence: "Ctrl+W"; onActivated: win.closeTab(win.currentIndex) }
    Shortcut { sequence: "Ctrl+Tab"; onActivated: win.show((win.currentIndex + 1) % win.panes.length) }
    Shortcut { sequence: "Ctrl+Shift+Tab"; onActivated: win.show((win.currentIndex - 1 + win.panes.length) % win.panes.length) }
    Shortcut { sequence: "Ctrl+L"; onActivated: address.edit() }
    Shortcut { sequence: "Alt+Left"; onActivated: win.current?.goBack() }
    Shortcut { sequence: "Alt+Right"; onActivated: win.current?.goForward() }
    Shortcut { sequences: ["Alt+Up", "Backspace"]; enabled: !address.editing; onActivated: win.current?.up() }
    Shortcut { sequence: "Ctrl+1"; onActivated: { if (win.current) win.current.mode = "grid" } }
    Shortcut { sequence: "Ctrl+2"; onActivated: { if (win.current) win.current.mode = "list" } }
    Shortcut { sequence: "Ctrl+3"; onActivated: { if (win.current) win.current.mode = "columns" } }
    Shortcut { sequence: "Ctrl+H"; onActivated: { if (win.current) win.current.hidden = !win.current.hidden } }
    Shortcut { sequence: "Ctrl+A"; enabled: !address.editing; onActivated: win.current?.selectAll() }
    Shortcut { sequence: "Ctrl+D"; onActivated: win.toggleBookmark() }
    Shortcut { sequence: "F5"; onActivated: win.current?.load() }
}
