// WhiteRose Files: tabs, the sidebar, the address bar, and the current
// folder as a grid, a list or columns.
//
// Tabs: drag one sideways to reorder it, down out of the bar to give it a
// window of its own; right-click for duplicate / move to new window / close
// others. Ctrl+T new, Ctrl+W close, Ctrl+Shift+T reopen closed,
// Ctrl+Tab / Ctrl+PgUp / Ctrl+PgDn switch, Ctrl+Shift+PgUp / PgDn move.
// Ctrl+L type a path, Alt+Left / Alt+Right back and forward, Alt+Up or
// Backspace up, Ctrl+1 / 2 / 3 grid, list, columns, Ctrl+H hidden files,
// Ctrl+A all, Ctrl+D bookmark this folder, F5 reload.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F
import "Tabs.js" as Tabs

AppWindow {
    id: win
    title: current ? current.title + " — Files" : "Files"
    visible: true
    width: 1040; height: 680

    // {tabs: [snapshot], current} for a window made by Session.open();
    // the first window asks Session.start().
    property var initial: null
    property var panes: []
    property int currentIndex: 0
    readonly property var current: panes[currentIndex] ?? null

    Component {
        id: paneComponent
        Pane {
            anchors.fill: parent
            visible: false
            onOpenFile: entry => Qt.openUrlExternally("file://" + entry.path)
            onChanged: Session.changed()
        }
    }
    // A tab from a snapshot ({path} at least; "" is home), at `at` (the end).
    function newTab(tab, at) {
        const props = {};
        for (const k of ["path", "back", "forward", "mode", "sortBy", "descending", "hidden"])
            if (tab && tab[k] !== undefined && tab[k] !== "") props[k] = tab[k];
        // A plain new tab looks like the one you're in.
        if (!props.mode && win.current) { props.mode = win.current.mode; props.hidden = win.current.hidden }
        const p = paneComponent.createObject(stack, props);
        const i = at ?? win.panes.length;
        const list = win.panes.slice();
        list.splice(i, 0, p);
        win.panes = list;
        win.show(i);
        Session.changed();
    }
    function show(i) {
        if (i < 0 || i >= win.panes.length) return;
        win.currentIndex = i;
        win.panes.forEach((p, k) => p.visible = k === i);
        win.panes[i].focusView();
    }
    // Takes tab i out (destroyed unless `keep`); the window closes with its last tab.
    function takeTab(i, keep) {
        if (win.panes.length <= 1) { win.close(); return }
        const p = win.panes[i];
        const next = Tabs.afterClose(win.panes.length, i, win.currentIndex);
        win.panes = win.panes.filter((_, k) => k !== i);
        if (!keep) p.destroy();
        win.show(next);
        Session.changed();
    }
    function closeTab(i) {
        if (win.panes.length > 1) Session.forget(win.panes[i].snapshot(), i);
        win.takeTab(i, false);
    }
    function closeOthers(i) {
        const keep = win.panes[i];
        for (let k = win.panes.length - 1; k >= 0; k--) if (win.panes[k] !== keep) win.closeTab(k);
    }
    function reopenTab() {
        const r = Session.reopen();
        if (r) win.newTab(r.tab, Math.min(r.index, win.panes.length));
    }
    function moveTab(from, to) {
        const cur = win.current;
        win.panes = Tabs.move(win.panes, from, to);
        win.currentIndex = win.panes.indexOf(cur);
        Session.changed();
    }
    function duplicateTab(i) { win.newTab(win.panes[i].snapshot(), i + 1) }
    function tearOff(i) {
        if (win.panes.length <= 1) return;
        const snap = win.panes[i].snapshot();
        win.takeTab(i, false);
        Session.open({ tabs: [snap], current: 0 });
    }
    function snapshot() { return { tabs: win.panes.map(p => p.snapshot()), current: win.currentIndex } }

    Component.onCompleted: {
        Session.add(win);
        const w = win.initial ?? Session.start();
        for (const t of w.tabs) win.newTab(t);
        win.show(Math.min(w.current ?? 0, win.panes.length - 1));
    }
    onClosing: {
        Session.remove(win);
        // Windows made later go away for good; the first one is the engine's.
        if (win.initial) Qt.callLater(() => win.destroy());
    }

    // ---- title bar: tabs ---------------------------------------------------------------
    // One mouse area over the whole strip does the clicking and dragging:
    // the tabs under it are rebuilt as they're reordered.
    toolbar: [
        Item {
            id: strip
            anchors.verticalCenter: parent.verticalCenter
            width: tabRow.width + 4 + plus.width
            height: 30
            property int hover: -1
            property int dragging: -1
            property bool away: false
            property real grabX: 0

            function at(x) {
                for (let k = 0; k < tabs.count; k++) {
                    const it = tabs.itemAt(k);
                    if (it && x >= it.x && x < it.x + it.width) return k;
                }
                return -1;
            }
            function onClose(k, x) {
                const it = tabs.itemAt(k);
                return win.panes.length > 1 && it && x > it.x + it.width - 26;
            }

            Row {
                id: tabRow
                spacing: 4
                Repeater {
                    id: tabs
                    model: win.panes
                    delegate: Rectangle {
                        required property var modelData
                        required property int index
                        readonly property bool on: index === win.currentIndex
                        height: 30
                        width: Math.min(200, label.implicitWidth + (closeIcon.visible ? 46 : 26))
                        radius: 9
                        color: on || strip.hover === index ? Theme.surface1 : "transparent"
                        border { width: on ? 1 : 0; color: Theme.surface2 }
                        opacity: strip.away && strip.dragging === index ? 0.45 : 1
                        Text {
                            id: label
                            anchors { left: parent.left; leftMargin: 13; verticalCenter: parent.verticalCenter }
                            text: modelData.title
                            width: Math.min(implicitWidth, 140)
                            elide: Text.ElideMiddle
                            color: parent.on ? Theme.text : Theme.muted
                            font { family: Theme.font; pointSize: Theme.pt(12); weight: parent.on ? Font.Medium : Font.Normal }
                        }
                        Icon {
                            id: closeIcon
                            visible: win.panes.length > 1
                            anchors { right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
                            name: "x"; size: 11
                            color: strip.hover === index ? Theme.text : Theme.faint
                        }
                    }
                }
            }
            IconButton {
                id: plus
                anchors { left: tabRow.right; leftMargin: 4; verticalCenter: parent.verticalCenter }
                width: 28; height: 28; icon: "plus"; iconSize: 13; fill: "transparent"
                onClicked: win.newTab({ path: win.current?.path ?? "" })
            }
            MouseArea {
                anchors { left: parent.left; top: parent.top; bottom: parent.bottom; right: plus.left }
                hoverEnabled: true
                preventStealing: true
                acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton
                cursorShape: strip.away ? Qt.DragMoveCursor : Qt.ArrowCursor
                onExited: strip.hover = -1
                onPressed: m => {
                    const k = strip.at(m.x);
                    if (k < 0) { m.accepted = m.button === Qt.LeftButton; if (m.accepted) win.startSystemMove(); return }
                    if (m.button !== Qt.LeftButton) return;
                    win.show(k);
                    strip.dragging = k;
                    strip.grabX = m.x;
                }
                onPositionChanged: m => {
                    if (strip.dragging < 0) { strip.hover = strip.at(m.x); return }
                    const below = m.y - strip.height;
                    strip.away = win.panes.length > 1 && below > Tabs.tearOff;
                    if (strip.away || Math.abs(m.x - strip.grabX) < 6) return;
                    const geo = [];
                    for (let k = 0; k < tabs.count; k++) { const it = tabs.itemAt(k); geo.push([it.x, it.width]) }
                    const to = Tabs.slot(geo, strip.dragging, m.x);
                    if (to !== strip.dragging) { win.moveTab(strip.dragging, to); strip.dragging = to }
                }
                onReleased: m => {
                    const k = strip.dragging, away = strip.away, moved = Math.abs(m.x - strip.grabX) >= 6;
                    strip.dragging = -1;
                    strip.away = false;
                    if (k < 0) return;
                    if (away) win.tearOff(k);
                    else if (!moved && strip.onClose(k, m.x)) win.closeTab(k);
                }
                onClicked: m => {
                    const k = strip.at(m.x);
                    if (k < 0) return;
                    if (m.button === Qt.MiddleButton) win.closeTab(k);
                    else if (m.button === Qt.RightButton) { tabMenu.tab = k; tabMenu.openAt(m.x, strip.height + 4) }
                }
                onDoubleClicked: m => { if (strip.at(m.x) < 0) win.toggleMaximized() }
                Menu {
                    id: tabMenu
                    property int tab: 0
                    items: [
                        { label: "New tab", shortcut: "Ctrl+T", run: () => win.newTab({ path: win.current?.path ?? "" }) },
                        { label: "Duplicate", run: () => win.duplicateTab(tabMenu.tab) },
                        { label: "Move to new window", enabled: win.panes.length > 1, run: () => win.tearOff(tabMenu.tab) },
                        { separator: true },
                        { label: "Reopen closed tab", shortcut: "Ctrl+Shift+T", enabled: Session.closed.length > 0, run: () => win.reopenTab() },
                        { label: "Close other tabs", enabled: win.panes.length > 1, run: () => win.closeOthers(tabMenu.tab) },
                        { label: "Close tab", shortcut: "Ctrl+W", run: () => win.closeTab(tabMenu.tab) },
                    ]
                }
            }
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
            onClicked: viewMenu.openAt(width - viewMenu.width, height + 4)
            Menu {
                id: viewMenu
                readonly property bool bookmarked: sidebar.places.bookmarks.some(b => b.path === win.current?.path)
                items: [
                    { label: "New tab", shortcut: "Ctrl+T", run: () => win.newTab({ path: win.current?.path ?? "" }) },
                    { label: "New window", shortcut: "Ctrl+N", run: () => Session.open({ tabs: [{ path: win.current?.path ?? "" }], current: 0 }) },
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
    Shortcut { sequence: "Ctrl+T"; onActivated: win.newTab({ path: win.current?.path ?? "" }) }
    Shortcut { sequence: "Ctrl+Shift+T"; onActivated: win.reopenTab() }
    Shortcut { sequence: "Ctrl+PgDown"; onActivated: win.show((win.currentIndex + 1) % win.panes.length) }
    Shortcut { sequence: "Ctrl+PgUp"; onActivated: win.show((win.currentIndex - 1 + win.panes.length) % win.panes.length) }
    Shortcut { sequence: "Ctrl+Shift+PgDown"; onActivated: { if (win.currentIndex < win.panes.length - 1) win.moveTab(win.currentIndex, win.currentIndex + 1) } }
    Shortcut { sequence: "Ctrl+Shift+PgUp"; onActivated: { if (win.currentIndex > 0) win.moveTab(win.currentIndex, win.currentIndex - 1) } }
    Shortcut { sequence: "Ctrl+N"; onActivated: Session.open({ tabs: [{ path: win.current?.path ?? "" }], current: 0 }) }
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
