// WhiteRose Files: tabs, the sidebar, the address bar, and the current
// folder as a grid, a list or columns, or two folders in a split tab.
//
// Tabs: drag one sideways to reorder it; out of the bar it's a drag any
// Files window's title bar takes (another process's too), or dropped
// anywhere else, a window of its own. Right-click for duplicate / move to
// new window / close others. Ctrl+T new, Ctrl+W close, Ctrl+Shift+T reopen
// closed, Ctrl+Tab / Ctrl+PgUp / Ctrl+PgDn switch, Ctrl+Shift+PgUp / PgDn move.
// Split: F3 on and off, Tab the other side, F5 / F6 copy / move the
// selection there (F5 or Ctrl+R reload when not split).
// Ctrl+L type a path, Alt+Left / Alt+Right back and forward, Alt+Up or
// Backspace up, Ctrl+1 / 2 / 3 grid, list, columns, Ctrl+H hidden files,
// Ctrl+A all, Ctrl+D bookmark this folder. Typing filters the folder;
// Ctrl+F searches (this folder and below, home, or everywhere).
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F
import "Tabs.js" as Tabs
import "Drops.js" as Drops

AppWindow {
    id: win
    title: current ? current.title + " — Files" : "Files"
    visible: true
    width: 1040; height: 680

    // {tabs: [snapshot], current} for a window made by Session.open();
    // the first window asks Session.start().
    property var initial: null
    // The tabs (TabView: a folder, or two when split).
    property var tabs: []
    property int currentIndex: 0
    // Set by Session.add (tabs dragged between windows say which they left).
    property int uid: 0
    readonly property var tab: win.tabs[win.currentIndex] ?? null
    // The folder in use: the tab's, or the side in use of a split one.
    readonly property var current: win.tab?.current ?? null

    Component {
        id: tabComponent
        TabView {
            anchors.fill: parent
            visible: false
            onOpenFile: entry => Qt.openUrlExternally("file://" + entry.path)
            onMenuAt: (entry, x, y) => win.showMenu(entry, x, y)
            onDropped: (paths, dest) => win.drop(paths, dest)
            onChanged: Session.changed()
        }
    }
    // A tab from a snapshot ({path} at least, "" for home; split ones have
    // `second`), at `at` (the end). How a folder is shown is up to it (views.rs).
    function newTab(snap, at) {
        const p = tabComponent.createObject(stack, { initial: snap ?? {} });
        const i = at ?? win.tabs.length;
        const list = win.tabs.slice();
        list.splice(i, 0, p);
        win.tabs = list;
        win.show(i);
        Session.changed();
    }
    function show(i) {
        if (i < 0 || i >= win.tabs.length) return;
        win.currentIndex = i;
        win.tabs.forEach((p, k) => p.visible = k === i);
        win.tabs[i].focusView();
    }
    // Takes tab i out (destroyed unless `keep`); the window closes with its last tab.
    function takeTab(i, keep) {
        if (win.tabs.length <= 1) { win.close(); return }
        const p = win.tabs[i];
        const next = Tabs.afterClose(win.tabs.length, i, win.currentIndex);
        win.tabs = win.tabs.filter((_, k) => k !== i);
        if (!keep) p.destroy();
        win.show(next);
        Session.changed();
    }
    function closeTab(i) {
        if (win.tabs.length > 1) Session.forget(win.tabs[i].snapshot(), i);
        win.takeTab(i, false);
    }
    function closeOthers(i) {
        const keep = win.tabs[i];
        for (let k = win.tabs.length - 1; k >= 0; k--) if (win.tabs[k] !== keep) win.closeTab(k);
    }
    function reopenTab() {
        const r = Session.reopen();
        if (r) win.newTab(r.tab, Math.min(r.index, win.tabs.length));
    }
    function moveTab(from, to) {
        const cur = win.tab;
        win.tabs = Tabs.move(win.tabs, from, to);
        win.currentIndex = win.tabs.indexOf(cur);
        Session.changed();
    }
    function duplicateTab(i) { win.newTab(win.tabs[i].snapshot(), i + 1) }
    function tearOff(i) {
        if (win.tabs.length <= 1) return;
        const snap = win.tabs[i].snapshot();
        win.takeTab(i, false);
        Session.open({ tabs: [snap], current: 0 });
    }
    function snapshot() { return { tabs: win.tabs.map(p => p.snapshot()), current: win.currentIndex } }

    Component.onCompleted: {
        Session.add(win);
        const w = win.initial ?? Session.start();
        for (const t of w.tabs) win.newTab(t);
        win.show(Math.min(w.current ?? 0, win.tabs.length - 1));
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
            // Where a tab dragged from any window would land (-1: none over).
            property int landing: -1
            function geometry() {
                const geo = [];
                for (let k = 0; k < tabRepeater.count; k++) { const it = tabRepeater.itemAt(k); geo.push([it.x, it.width]) }
                return geo;
            }

            // A tab out of the bar is a real drag: another window's bar
            // (this process or another) takes it; dropped anywhere else, it
            // gets a window of its own.
            Item {
                id: tabCarry
                property int index: -1
                // It came back to this bar (placed already).
                property bool home: false
                property string id: ""
                width: 1; height: 1
                Drag.dragType: Drag.Automatic
                Drag.supportedActions: Qt.MoveAction | Qt.CopyAction
                Drag.proposedAction: Qt.MoveAction
                Drag.onDragFinished: action => {
                    const k = tabCarry.index, home = tabCarry.home, taken = Fs.tabTaken(tabCarry.id);
                    tabCarry.Drag.active = false;
                    tabCarry.index = -1;
                    tabCarry.home = false;
                    strip.dragging = -1;
                    strip.away = false;
                    if (k < 0 || k >= win.tabs.length || home) return;
                    // Taken by another window's bar (which action Wayland
                    // reports across processes varies, so any counts), or
                    // dropped nowhere: a window of its own.
                    if (taken || action === Qt.MoveAction) win.takeTab(k, false);
                    else if (action === Qt.IgnoreAction) win.tearOff(k);
                }
                function start(k, hotX, hotY) {
                    const it = tabRepeater.itemAt(k);
                    if (!it) return;
                    tabCarry.index = k;
                    tabCarry.id = Session.token + win.uid + "x" + Date.now().toString(36);
                    tabCarry.Drag.mimeData = { [Tabs.mime]: Tabs.pack(Session.token, win.uid, k, win.tabs[k].snapshot(), tabCarry.id) };
                    tabCarry.Drag.hotSpot = Qt.point(hotX, hotY);
                    it.grabToImage(r => {
                        tabCarry.Drag.imageSource = r.url;
                        tabCarry.Drag.active = true;
                    });
                }
            }
            // Anywhere on the title bar takes a tab.
            DropArea {
                id: tabLanding
                parent: win.titleBar
                anchors.fill: parent
                keys: [Tabs.mime]
                function stripX(x) { return strip.mapFromItem(tabLanding, x, 0).x }
                onPositionChanged: drag => strip.landing = Tabs.insertAt(strip.geometry(), tabLanding.stripX(drag.x))
                onExited: strip.landing = -1
                onDropped: drop => {
                    const at = Tabs.insertAt(strip.geometry(), tabLanding.stripX(drop.x));
                    strip.landing = -1;
                    const d = Tabs.unpack(drop.getDataAsString(Tabs.mime));
                    if (!d) return;
                    if (d.token === Session.token && d.win === win.uid) {
                        // Back on its own bar: just a new place.
                        win.moveTab(d.index, at > d.index ? at - 1 : at);
                        tabCarry.home = true;
                        drop.accept(Qt.MoveAction);
                    } else {
                        win.newTab(d.tab, at);
                        if (d.id) Fs.takeTab(d.id);
                        win.requestActivate();
                        drop.accept(Qt.MoveAction);
                    }
                }
            }
            // Where it would land.
            Rectangle {
                visible: strip.landing >= 0
                z: 5
                width: 2; height: parent.height; radius: 1
                color: Theme.tint(0.9)
                x: {
                    const g = strip.geometry();
                    if (strip.landing < 0 || g.length === 0) return 0;
                    return strip.landing < g.length ? g[strip.landing][0] - 3 : g[g.length - 1][0] + g[g.length - 1][1] + 1;
                }
            }

            function at(x) {
                for (let k = 0; k < tabRepeater.count; k++) {
                    const it = tabRepeater.itemAt(k);
                    if (it && x >= it.x && x < it.x + it.width) return k;
                }
                return -1;
            }
            function onClose(k, x) {
                const it = tabRepeater.itemAt(k);
                return win.tabs.length > 1 && it && x > it.x + it.width - 26;
            }

            Row {
                id: tabRow
                spacing: 4
                Repeater {
                    id: tabRepeater
                    model: win.tabs
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
                        // Files held over a tab open it (to drop in its
                        // folder, or further in); dropped on it: into its folder.
                        FolderDrop {
                            id: tabDrop
                            anchors.fill: parent
                            dest: modelData.current.path
                            onFilesDropped: (p, d) => win.drop(p, d)
                        }
                        Timer { running: tabDrop.over && !parent.on; interval: 600; onTriggered: win.show(index) }
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
                            visible: win.tabs.length > 1
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
                    // Out of the bar: from here it's a real drag (tabCarry).
                    if (below > Tabs.tearOff || m.y < -Tabs.tearOff) {
                        if (!strip.away && tabCarry.index < 0) {
                            strip.away = true;
                            const it = tabRepeater.itemAt(strip.dragging);
                            tabCarry.start(strip.dragging, Math.max(0, strip.grabX - it.x), 15);
                        }
                        return;
                    }
                    if (Math.abs(m.x - strip.grabX) < 6) return;
                    const to = Tabs.slot(strip.geometry(), strip.dragging, m.x);
                    if (to !== strip.dragging) { win.moveTab(strip.dragging, to); strip.dragging = to }
                }
                onReleased: m => {
                    // Gone out of the bar: the drag finishes it (tabCarry).
                    if (strip.away) return;
                    const k = strip.dragging, moved = Math.abs(m.x - strip.grabX) >= 6;
                    strip.dragging = -1;
                    if (k < 0) return;
                    if (!moved && strip.onClose(k, m.x)) win.closeTab(k);
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
                        { label: "Move to new window", enabled: win.tabs.length > 1, run: () => win.tearOff(tabMenu.tab) },
                        { separator: true },
                        { label: "Reopen closed tab", shortcut: "Ctrl+Shift+T", enabled: Session.closed.length > 0, run: () => win.reopenTab() },
                        { label: "Close other tabs", enabled: win.tabs.length > 1, run: () => win.closeOthers(tabMenu.tab) },
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
        onDropped: (paths, dest) => win.drop(paths, dest)
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
            IconButton { width: 32; height: 32; icon: "chevUp"; fill: "transparent"; opacity: Fs.up(win.current?.path ?? "/") !== "" ? 1 : 0.35; onClicked: win.current?.up() }
        }
        AddressBar {
            id: address
            visible: !(win.current?.searching ?? false)
            anchors { left: arrows.right; leftMargin: 8; right: emptyTrash.visible ? emptyTrash.left : views.left; rightMargin: 10; verticalCenter: parent.verticalCenter }
            path: win.current?.path ?? "/"
            hidden: win.current?.hidden ?? false
            onGo: p => { win.current?.go(p); win.current?.focusView() }
            onDone: win.current?.focusView()
            onDropped: (paths, dest) => win.drop(paths, dest)
            z: 5
        }
        // Searching: what for, in place of where you are.
        Field {
            id: searchField
            visible: win.current?.searching ?? false
            anchors { left: arrows.right; leftMargin: 8; right: views.left; rightMargin: 10; verticalCenter: parent.verticalCenter }
            width: 0; height: 34
            placeholder: "Search"
            onTextChanged: if (searchField.visible && searchField.text !== (win.current?.search?.text ?? "")) searchSoon.restart()
            onAccepted: { searchSoon.stop(); win.current?.find({ text: searchField.text }); win.current?.focusView() }
            Keys.onEscapePressed: win.closeSearch()
            Keys.onDownPressed: win.current?.focusView()
            Timer { id: searchSoon; interval: 180; onTriggered: win.current?.find({ text: searchField.text }) }
        }
        Button {
            id: emptyTrash
            visible: win.current?.inTrash ?? false
            anchors { right: views.left; rightMargin: 10; verticalCenter: parent.verticalCenter }
            text: "Empty Trash"
            kind: "danger"
            enabled: (win.current?.entries.length ?? 0) > 0
            onClicked: if (enabled) win.askEmpty()
        }
        Segmented {
            id: views
            anchors { right: searchButton.left; rightMargin: 6; verticalCenter: parent.verticalCenter }
            value: win.current?.mode ?? "grid"
            options: [{ label: "Grid", value: "grid" }, { label: "List", value: "list" }, { label: "Columns", value: "columns" }]
            onPicked: v => { if (win.current) { win.current.setView({ mode: v }); Qt.callLater(() => win.current.focusView()) } }
        }
        IconButton {
            id: searchButton
            anchors { right: menuButton.left; rightMargin: 2; verticalCenter: parent.verticalCenter }
            width: 32; height: 32; icon: win.current?.searching ? "x" : "search"; fill: "transparent"
            onClicked: win.current?.searching ? win.closeSearch() : win.openSearch("")
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
                    { label: win.tab?.split ? "Close split" : "Split", shortcut: "F3", run: () => win.tab.setSplit(!win.tab.split) },
                    ...(win.tab?.split ? [{ label: win.tab.vertical ? "Side by side" : "One above the other", run: () => win.tab.setVertical(!win.tab.vertical) }] : []),
                    { label: (Session.hidden ? "Hide" : "Show") + " hidden files", shortcut: "Ctrl+H", run: () => Session.setHidden(!Session.hidden) },
                    { label: viewMenu.bookmarked ? "Remove bookmark" : "Bookmark this folder", shortcut: "Ctrl+D", run: () => win.toggleBookmark() },
                    { separator: true },
                    { label: "Sort by name", run: () => win.current.setView({ sortBy: "name", descending: false }) },
                    { label: "Sort by size", run: () => win.current.setView({ sortBy: "size", descending: true }) },
                    { label: "Sort by date modified", run: () => win.current.setView({ sortBy: "modified", descending: true }) },
                    { label: "Sort by kind", run: () => win.current.setView({ sortBy: "kind", descending: false }) },
                    { separator: true },
                    { label: (win.current?.viewInfo.subfolders ? "Stop using this view in subfolders" : "Use this view in subfolders"),
                      run: () => { Fs.setViewForSubfolders(win.current.path, !win.current.viewInfo.subfolders); win.refreshViews() } },
                    { label: "Use this view everywhere",
                      run: () => { Fs.setDefaultView(win.current.path, JSON.stringify({ mode: win.current.mode, sortBy: win.current.sortBy, descending: win.current.descending })); win.refreshViews() } },
                    { label: "Reset this folder's view", enabled: win.current?.viewInfo.own ?? false,
                      run: () => { Fs.resetView(win.current.path); win.refreshViews() } },
                ]
            }
        }
        Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.surface1 }
    }
    // Where views come from changed: every tab in every window shows its folder again.
    function refreshViews() { for (const w of Session.windows) for (const t of w.tabs) t.refreshView() }
    function toggleBookmark() {
        if (!win.current) return;
        const on = !sidebar.places.bookmarks.some(b => b.path === win.current.path);
        Fs.setBookmark(win.current.path, on);
        sidebar.refresh();
    }

    // ---- searching: where and what ------------------------------------------------------------
    Item {
        id: searchRow
        visible: win.current?.searching ?? false
        anchors { left: sidebar.right; right: parent.right; top: nav.bottom }
        height: searchRow.visible ? 46 : 0
        readonly property var ask: win.current?.search ?? ({})
        Row {
            anchors { left: parent.left; leftMargin: 14; verticalCenter: parent.verticalCenter }
            spacing: 8
            Segmented {
                value: searchRow.ask.scope ?? "here"
                options: [{ label: "This folder", value: "here" }, { label: "Home", value: "home" }, { label: "Everywhere", value: "everywhere" }]
                onPicked: v => win.current?.find({ scope: v })
            }
            Choice {
                value: searchRow.ask.kind ?? ""
                options: [{ label: "Any type", value: "" }, { label: "Folders", value: "folder" }, { label: "Documents", value: "document" },
                          { label: "Images", value: "image" }, { label: "Audio", value: "audio" }, { label: "Video", value: "video" },
                          { label: "Archives", value: "archive" }, { label: "Code", value: "code" }]
                onPicked: v => win.current?.find({ kind: v })
            }
            Choice {
                value: searchRow.ask.size ?? ""
                options: [{ label: "Any size", value: "" }, { label: "Under 1 MB", value: "small" }, { label: "1–100 MB", value: "medium" },
                          { label: "100 MB–1 GB", value: "large" }, { label: "Over 1 GB", value: "huge" }]
                onPicked: v => win.current?.find({ size: v })
            }
            Choice {
                value: searchRow.ask.modified ?? ""
                options: [{ label: "Any time", value: "" }, { label: "Today", value: "today" }, { label: "Past week", value: "week" },
                          { label: "Past month", value: "month" }, { label: "Past year", value: "year" }]
                onPicked: v => win.current?.find({ modified: v })
            }
        }
        // The index (home and drives): how fresh, and freshen it.
        Row {
            visible: (searchRow.ask.scope ?? "here") !== "here"
            anchors { right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
            spacing: 4
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: Search.indexing ? "Indexing…" : Search.indexedAt > 0 ? "Indexed " + F.date(Search.indexedAt, Date.now()).replace(/^Today /, "at ") : "Not indexed yet"
                color: Theme.faint
                font { family: Theme.font; pointSize: Theme.pt(11) }
            }
            IconButton {
                visible: !Search.indexing
                width: 26; height: 26; icon: "restart"; iconSize: 12; fill: "transparent"
                onClicked: Search.reindex()
            }
        }
        Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.surface1 }
    }
    // Index done: searches of home and the drives see what it found.
    Connections {
        target: Search
        function onIndexingChanged() {
            if (Search.indexing) return;
            for (const t of win.tabs) for (const p of t.panes) if (p.searching && p.search.scope !== "here") p.find({});
        }
    }
    // A search result, in its folder.
    function showInFolder(entry) {
        const p = win.current;
        const folder = entry.path.substring(0, entry.path.lastIndexOf("/")) || "/";
        p.go(folder);
        p.selectPaths([entry.path]);
        p.focusView();
    }
    function openSearch(text) {
        if (!win.current) return;
        win.current.find({ text: text });
        searchField.text = text;
        Qt.callLater(() => searchField.focusField());
    }
    function closeSearch() {
        win.current?.endSearch();
        win.current?.focusView();
    }
    // The box shows the search of the tab and side in use (and one started
    // from the type-to-filter).
    onCurrentChanged: searchField.text = win.current?.search?.text ?? ""
    Connections {
        target: win.current
        function onSearchChanged() {
            const t = win.current?.search?.text ?? "";
            if (!searchField.editing && searchField.text !== t) searchField.text = t;
        }
    }

    // ---- the folder -----------------------------------------------------------------------
    Item {
        id: stack
        anchors { left: sidebar.right; right: parent.right; top: searchRow.bottom; bottom: status.top }

        // Right-click on files (`entry` set) or on the folder's empty space.
        Menu {
            id: itemMenu
            property var entry: null
            readonly property var sel: win.current?.selection ?? []
            readonly property bool many: sel.length > 1
            readonly property bool trashed: win.current?.inTrash ?? false
            items: itemMenu.trashed ? (!itemMenu.entry ? [
                { label: "Empty Trash…", danger: true, enabled: (win.current?.entries.length ?? 0) > 0, run: () => win.askEmpty() },
                { label: "Select all", shortcut: "Ctrl+A", run: () => win.current.selectAll() },
            ] : [
                { label: "Restore", run: () => win.restore(itemMenu.sel.map(e => e.path)) },
                { label: "Delete permanently…", shortcut: "Delete", danger: true, run: () => win.askDelete(itemMenu.sel.map(e => e.path), "purge") },
                { separator: true },
                { label: "Properties", shortcut: "Alt+Enter", enabled: !itemMenu.many, run: () => win.properties() },
            ]) : !itemMenu.entry ? [
                { label: "New folder", shortcut: "Ctrl+Shift+N", run: () => win.askName("folder") },
                { label: "New file", run: () => win.askName("file") },
                { separator: true },
                { label: "Paste", shortcut: "Ctrl+V", enabled: win.clipFiles.length > 0, run: () => win.paste(win.current.path) },
                { label: "Select all", shortcut: "Ctrl+A", run: () => win.current.selectAll() },
                { separator: true },
                { label: (Session.hidden ? "Hide" : "Show") + " hidden files", shortcut: "Ctrl+H", run: () => Session.setHidden(!Session.hidden) },
                { label: "Properties", run: () => props.show([win.current.path]) },
            ] : [
                { label: "Open", shortcut: "Enter", run: () => win.current.activate(itemMenu.entry) },
                ...(win.current?.searching ? [{ label: "Show in folder", run: () => win.showInFolder(itemMenu.entry) }] : []),
                ...(itemMenu.entry.dir && !itemMenu.many ? [{ label: "Open in new tab", run: () => win.newTab({ path: itemMenu.entry.path }, win.currentIndex + 1) }] : []),
                { separator: true },
                { label: "Cut", shortcut: "Ctrl+X", run: () => win.toClipboard(true) },
                { label: "Copy", shortcut: "Ctrl+C", run: () => win.toClipboard(false) },
                ...(itemMenu.entry.dir && !itemMenu.many ? [{ label: "Paste into folder", enabled: win.clipFiles.length > 0, run: () => win.paste(itemMenu.entry.path) }] : []),
                { label: "Duplicate", run: () => win.run("copy", itemMenu.sel.map(e => e.path), win.current.path) },
                ...(win.tab?.other ? [
                    { label: "Copy to other side", shortcut: "F5", run: () => win.toOther("copy") },
                    { label: "Move to other side", shortcut: "F6", run: () => win.toOther("move") },
                ] : []),
                { separator: true },
                { label: "Rename…", shortcut: "F2", enabled: !itemMenu.many, run: () => win.askName("rename") },
                { label: "Move to Trash", shortcut: "Delete", run: () => win.trash() },
                { label: "Delete permanently…", shortcut: "Shift+Del", danger: true, run: () => win.askDelete(itemMenu.sel.map(e => e.path)) },
                { separator: true },
                { label: "Properties", shortcut: "Alt+Enter", run: () => win.properties() },
            ]
        }

        Transfers {
            id: transfers
            anchors { right: parent.right; bottom: parent.bottom; margins: 14 }
            z: 20
            onDeleteInstead: paths => win.askDelete(paths)
        }
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
                if (win.notice !== "") return win.notice;
                const p = win.current;
                if (!p) return "";
                if (p.searching) {
                    const n = p.results.length;
                    return (n === 1 ? "1 result" : n + " results") + (p.searchDone ? "" : " · looking…");
                }
                const n = p.shownEntries.length, sel = p.selection;
                const items = n + (n === 1 ? " item" : " items");
                if (sel.length === 0) return items;
                const bytes = sel.filter(e => !e.dir).reduce((a, e) => a + e.size, 0);
                return items + " · " + sel.length + " selected" + (bytes > 0 ? " (" + F.size(bytes) + ")" : "");
            }
            color: win.notice !== "" ? Theme.rose : Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(11) }
        }
    }

    // ---- file operations ---------------------------------------------------------------------
    // Files on the clipboard (checked when a menu opens or Paste is pressed).
    property var clipFiles: []
    property bool clipCut: false
    // Jobs started here, and the tab that wants their results selected.
    property var waiting: ({})
    readonly property bool typing: address.editing || nameDialog.visible || confirm.visible || props.visible || searchField.editing

    function readClipboard() {
        const c = JSON.parse(Ops.clipboard());
        win.clipFiles = c.paths;
        win.clipCut = c.cut;
    }
    function showMenu(entry, x, y) {
        win.readClipboard();
        itemMenu.entry = entry;
        Qt.callLater(() => itemMenu.openAt(Math.min(x, stack.width - itemMenu.width - 8), Math.min(y, stack.height - itemMenu.height - 8)));
    }
    function run(kind, paths, dest) {
        if (paths.length === 0) return;
        const id = Ops.start(kind, JSON.stringify(paths), dest ?? "");
        // The side showing where they went selects them when they're there.
        const shows = (win.tab?.panes ?? []).find(p => p.path === dest);
        if (id >= 0 && dest && shows) {
            const w = Object.assign({}, win.waiting);
            w[id] = shows;
            win.waiting = w;
        }
    }
    // Files dropped on folder `dest` (Drops.plan: copy or move, or trash).
    function drop(paths, dest) {
        const mods = Ops.modifiers();
        const plan = Drops.plan(paths, dest, (mods & Qt.ControlModifier) !== 0, (mods & Qt.ShiftModifier) !== 0,
                                Fs.sameDrive(paths[0], dest));
        if (plan) win.run(plan.kind, plan.paths, plan.kind === "trash" ? "" : dest);
    }
    // The selection to the folder on the other side of a split tab.
    function toOther(kind) {
        const sel = win.current?.selection ?? [];
        const other = win.tab?.other;
        if (sel.length === 0 || !other || other.inTrash) return;
        win.run(kind, sel.map(e => e.path), other.path);
    }
    function toClipboard(cut) {
        const sel = win.current?.selection ?? [];
        if (sel.length > 0) Ops.setClipboard(JSON.stringify(sel.map(e => e.path)), cut);
    }
    function paste(dest) {
        win.readClipboard();
        if (win.clipFiles.length === 0) return;
        win.run(win.clipCut ? "move" : "copy", win.clipFiles, dest);
        // Cut and pasted: they've moved, so the clipboard is done with them.
        if (win.clipCut) Ops.setClipboard("[]", false);
    }
    function trash() {
        const sel = win.current?.selection ?? [];
        // In the Trash, Delete means for good.
        if (win.current?.inTrash) win.askDelete(sel.map(e => e.path), "purge");
        else win.run("trash", sel.map(e => e.path));
    }
    // kind: "delete" (anywhere), "purge" (things in the Trash), "empty" (all of it).
    function askDelete(paths, kind) {
        if (paths.length === 0) return;
        confirm.paths = paths;
        confirm.kind = kind ?? "delete";
        // Shown by their own names (a trashed one's name in the Trash can differ).
        confirm.names = paths.map(p => (win.current?.entries.find(e => e.path === p)?.name) ?? p.substring(p.lastIndexOf("/") + 1));
        confirm.open();
    }
    function askEmpty() { win.askDelete((win.current?.entries ?? []).map(e => e.path), "empty") }
    function restore(paths) {
        const r = JSON.parse(Ops.restore(JSON.stringify(paths)));
        win.current?.load();
        if (r.failed.length > 0) {
            const name = r.failed[0][0].substring(r.failed[0][0].lastIndexOf("/") + 1);
            win.notice = "Couldn't restore “" + name + "”: " + r.failed[0][1];
        }
    }
    function properties() {
        const sel = win.current?.selection ?? [];
        // (The Trash itself has none.)
        if (sel.length === 0 && win.current?.inTrash) return;
        props.show(sel.length > 0 ? sel.map(e => e.path) : [win.current.path]);
    }
    // A line for the status bar, for a few seconds.
    property string notice: ""
    Timer { running: win.notice !== ""; interval: 6000; onTriggered: win.notice = "" }
    Properties { id: props; onClosed: win.current?.focusView() }
    // "rename", "folder" or "file".
    function askName(what) {
        const p = win.current;
        if (!p) return;
        const e = what === "rename" ? p.selection[0] : null;
        if (what === "rename" && !e) return;
        nameDialog.what = what;
        nameDialog.entry = e;
        nameDialog.error = "";
        nameField.text = e ? e.name : what === "folder" ? "New folder" : "New file";
        nameDialog.open();
    }
    function nameChosen() {
        const p = win.current, name = nameField.text;
        const r = JSON.parse(nameDialog.what === "rename" ? Ops.rename(nameDialog.entry.path, name)
                                                          : Ops.create(p.path, name, nameDialog.what === "file"));
        if (!r.ok) { nameDialog.error = r.error; return }
        nameDialog.close();
        p.selectPaths([r.path]);
        p.focusView();
    }
    Connections {
        target: Ops
        function onFinished(id, result) {
            const pane = win.waiting[id];
            if (!pane) return;
            const w = Object.assign({}, win.waiting);
            delete w[id];
            win.waiting = w;
            const r = JSON.parse(result);
            if (win.tabs.some(t => t.panes.includes(pane))) pane.selectPaths(r.done);
        }
    }

    // Name for a rename, a new folder or a new file.
    Popup {
        id: nameDialog
        property string what: "rename"
        property var entry: null
        property string error: ""
        parent: Overlay.overlay
        anchors.centerIn: parent
        width: 360
        modal: true
        padding: 20
        background: Rectangle { color: Theme.surface1; radius: 16; border { width: 1; color: Theme.surface2 } }
        Overlay.modal: Rectangle { color: Qt.rgba(0, 0, 0, 0.35) }
        onOpened: {
            nameField.focusField();
            const name = nameField.text;
            nameField.select(0, nameDialog.what === "rename" ? F.stemLength(name, nameDialog.entry.dir) : name.length);
        }
        onClosed: win.current?.focusView()
        contentItem: Column {
            spacing: 12
            Text {
                text: nameDialog.what === "rename" ? "Rename" : nameDialog.what === "folder" ? "New folder" : "New file"
                color: Theme.text
                font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
            }
            Field {
                id: nameField
                width: 320
                error: nameDialog.error !== ""
                onAccepted: Qt.callLater(win.nameChosen)
                onTextChanged: nameDialog.error = ""
                Keys.onEscapePressed: nameDialog.close()
            }
            Text {
                visible: nameDialog.error !== ""
                text: nameDialog.error
                color: Theme.rose
                font { family: Theme.font; pointSize: Theme.pt(11.5) }
            }
            Row {
                anchors.right: parent.right
                spacing: 8
                Button { text: "Cancel"; kind: "ghost"; onClicked: nameDialog.close() }
                Button {
                    text: nameDialog.what === "rename" ? "Rename" : "Create"
                    kind: "primary"
                    onClicked: win.nameChosen()
                }
            }
        }
    }

    // Deleting for good.
    Popup {
        id: confirm
        property var paths: []
        property var names: []
        property string kind: "delete"
        parent: Overlay.overlay
        anchors.centerIn: parent
        width: 380
        modal: true
        padding: 20
        background: Rectangle { color: Theme.surface1; radius: 16; border { width: 1; color: Theme.surface2 } }
        Overlay.modal: Rectangle { color: Qt.rgba(0, 0, 0, 0.35) }
        onOpened: deleteButton.forceActiveFocus()
        onClosed: win.current?.focusView()
        contentItem: Column {
            spacing: 12
            Text {
                width: 340
                text: confirm.kind === "empty" ? "Empty the Trash?"
                    : confirm.names.length === 1 ? "Delete “" + confirm.names[0] + "” permanently?"
                    : "Delete " + confirm.paths.length + " items permanently?"
                wrapMode: Text.Wrap
                color: Theme.text
                font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
            }
            Text {
                width: 340
                text: confirm.kind === "delete" ? "They won't go to the Trash, so they can't be brought back."
                    : confirm.kind === "empty" ? "Everything in it (" + confirm.paths.length + (confirm.paths.length === 1 ? " item" : " items") + ") is deleted for good."
                    : "They can't be brought back after this."
                wrapMode: Text.Wrap
                color: Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(12) }
            }
            Row {
                anchors.right: parent.right
                spacing: 8
                Button { text: "Cancel"; kind: "ghost"; onClicked: confirm.close() }
                Button {
                    id: deleteButton
                    text: confirm.kind === "empty" ? "Empty Trash" : "Delete"
                    kind: "danger"
                    // After the key is handled: closing in the middle of it crashes Qt.
                    Keys.onReturnPressed: e => { e.accepted = true; Qt.callLater(() => deleteButton.clicked()) }
                    Keys.onEscapePressed: confirm.close()
                    onClicked: { win.run(confirm.kind, confirm.paths); confirm.close() }
                }
            }
        }
    }

    // ---- keys -------------------------------------------------------------------------------
    Shortcut { sequence: "Ctrl+T"; onActivated: win.newTab({ path: win.current?.path ?? "" }) }
    Shortcut { sequence: "Ctrl+Shift+T"; onActivated: win.reopenTab() }
    Shortcut { sequence: "Ctrl+PgDown"; onActivated: win.show((win.currentIndex + 1) % win.tabs.length) }
    Shortcut { sequence: "Ctrl+PgUp"; onActivated: win.show((win.currentIndex - 1 + win.tabs.length) % win.tabs.length) }
    Shortcut { sequence: "Ctrl+Shift+PgDown"; onActivated: { if (win.currentIndex < win.tabs.length - 1) win.moveTab(win.currentIndex, win.currentIndex + 1) } }
    Shortcut { sequence: "Ctrl+Shift+PgUp"; onActivated: { if (win.currentIndex > 0) win.moveTab(win.currentIndex, win.currentIndex - 1) } }
    Shortcut { sequence: "Ctrl+N"; onActivated: Session.open({ tabs: [{ path: win.current?.path ?? "" }], current: 0 }) }
    Shortcut { sequence: "Ctrl+W"; onActivated: win.closeTab(win.currentIndex) }
    Shortcut { sequence: "Ctrl+Tab"; onActivated: win.show((win.currentIndex + 1) % win.tabs.length) }
    Shortcut { sequence: "Ctrl+Shift+Tab"; onActivated: win.show((win.currentIndex - 1 + win.tabs.length) % win.tabs.length) }
    Shortcut { sequence: "Ctrl+L"; onActivated: address.edit() }
    Shortcut { sequence: "Alt+Left"; onActivated: win.current?.goBack() }
    Shortcut { sequence: "Alt+Right"; onActivated: win.current?.goForward() }
    // (Backspace is the type-to-filter's while it has letters.)
    Shortcut { sequence: "Alt+Up"; enabled: !win.typing; onActivated: win.current?.up() }
    Shortcut { sequence: "Backspace"; enabled: !win.typing && (win.current?.filter ?? "") === "" && !(win.current?.searching ?? false); onActivated: win.current?.up() }
    Shortcut { sequence: "Ctrl+F"; onActivated: win.openSearch(win.current?.filter ?? "") }
    Shortcut { sequence: "Ctrl+1"; onActivated: win.current?.setView({ mode: "grid" }) }
    Shortcut { sequence: "Ctrl+2"; onActivated: win.current?.setView({ mode: "list" }) }
    Shortcut { sequence: "Ctrl+3"; onActivated: win.current?.setView({ mode: "columns" }) }
    Shortcut { sequence: "Ctrl+H"; onActivated: Session.setHidden(!Session.hidden) }
    Shortcut { sequence: "Ctrl+A"; enabled: !win.typing; onActivated: win.current?.selectAll() }
    Shortcut { sequence: "Ctrl+D"; onActivated: win.toggleBookmark() }
    // F5 / F6 copy / move to the other side of a split tab (reload when there's none).
    Shortcut { sequence: "F5"; enabled: !win.typing; onActivated: { if (win.tab?.other) win.toOther("copy"); else win.current?.load() } }
    Shortcut { sequence: "F6"; enabled: !win.typing && !!win.tab?.other; onActivated: win.toOther("move") }
    Shortcut { sequence: "F3"; enabled: !win.typing; onActivated: win.tab?.setSplit(!win.tab.split) }
    // Tab: the other side.
    Shortcut { sequence: "Tab"; enabled: !win.typing && !!win.tab?.other; onActivated: { win.tab.active = 1 - win.tab.active; win.tab.focusView() } }
    Shortcut { sequence: "Ctrl+R"; onActivated: win.current?.load() }
    Shortcut { sequence: "Ctrl+C"; enabled: !win.typing; onActivated: win.toClipboard(false) }
    Shortcut { sequence: "Ctrl+X"; enabled: !win.typing; onActivated: win.toClipboard(true) }
    Shortcut { sequence: "Ctrl+V"; enabled: !win.typing && !(win.current?.inTrash ?? false); onActivated: { if (win.current) win.paste(win.current.path) } }
    Shortcut { sequence: "Delete"; enabled: !win.typing; onActivated: win.trash() }
    Shortcut { sequence: "Shift+Del"; enabled: !win.typing; onActivated: win.askDelete((win.current?.selection ?? []).map(e => e.path), win.current?.inTrash ? "purge" : "delete") }
    Shortcut { sequence: "F2"; enabled: !win.typing && !(win.current?.inTrash ?? false); onActivated: win.askName("rename") }
    Shortcut { sequences: ["Alt+Return", "Alt+Enter"]; enabled: !win.typing; onActivated: win.properties() }
    Shortcut { sequence: "Ctrl+Shift+N"; enabled: !win.typing && !(win.current?.inTrash ?? false); onActivated: win.askName("folder") }
}
