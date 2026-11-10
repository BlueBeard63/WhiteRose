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
// Ctrl+L type a path, Ctrl+J jump to a folder by a few letters, Alt+Left / Alt+Right back and forward, Alt+Up or
// Backspace up, Ctrl+1 / 2 / 3 grid, list, columns, Ctrl+H hidden files,
// Ctrl+A all, Ctrl+D bookmark this folder. Typing filters the folder;
// Ctrl+F searches (this folder and below, home, or everywhere). Space looks
// closer at the selection, Alt+P shows details beside the folder, F4 opens
// a terminal here.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F
import "Tabs.js" as Tabs
import "Drops.js" as Drops
import "FilesKeys.js" as FilesKeys
import "FilesActions.js" as Actions

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
            onOpenFile: entry => win.openFile(entry)
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
    // This window's tabs swapped for `w`'s ({tabs, current}).
    function replaceTabs(w) {
        const old = win.tabs;
        win.tabs = [];
        for (const t of w.tabs) win.newTab(t);
        win.show(Math.min(w.current ?? 0, win.tabs.length - 1));
        for (const p of old) p.destroy();
        Session.changed();
    }
    // A workspace's windows: this one becomes its first (or, `fresh`, all
    // of them open as new windows).
    function openWorkspace(name, fresh) {
        const r = JSON.parse(Fs.openWorkspace(name));
        if (!r.ok) { win.notice = r.error; return }
        r.windows.forEach((w, i) => { if (i === 0 && !fresh) win.replaceTabs(w); else Session.open(w) });
        Session.reload(true);
    }
    // Saving what's open as a workspace (`old` "": a new one), or renaming `old`.
    function askWorkspace(old) {
        nameDialog.what = old ? "renameWorkspace" : "workspace";
        nameDialog.entry = old ? { name: old } : null;
        nameDialog.error = "";
        nameDialog.group = [];
        nameField.text = old || (win.current?.title ?? "Workspace");
        nameDialog.open();
    }
    // The ☰ › Workspaces menu (made again when workspaces change).
    property var workspaceMenu: workspaceItems()
    Connections { target: Session; function onWorkspacesChanged() { win.workspaceMenu = win.workspaceItems() } }
    function workspaceItems() {
        const all = JSON.parse(Fs.workspaces());
        return all.map(w => ({ label: w.name, items: [
            { label: "Open", run: () => win.openWorkspace(w.name) },
            { label: "Open in a new window", run: () => win.openWorkspace(w.name, true) },
            { label: "Save what's open as this", run: () => Session.saveWorkspace(w.name) },
            { separator: true },
            { label: "Rename…", run: () => win.askWorkspace(w.name) },
            { label: "Delete", danger: true, run: () => { Fs.deleteWorkspace(w.name); Session.workspacesChanged() } },
        ] })).concat(all.length > 0 ? [{ separator: true }] : []).concat([
            { label: "Save what's open…", run: () => win.askWorkspace("") },
            { label: "Start page", run: () => win.current?.go("start:") },
        ]);
    }

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
        // Hidden (Ctrl+B): the folder takes the whole width.
        width: Session.sidebar ? 210 : 0
        visible: width > 0
        clip: true
        Behavior on width { NumberAnimation { duration: 160; easing.type: Easing.OutCubic } }
        current: win.current?.path ?? ""
        onChosen: p => { win.current?.go(p); win.current?.focusView() }
        onDropped: (paths, dest) => win.drop(paths, dest)
        onTagMenu: (name, x, y) => { tagMenu.tag = name; tagMenu.openAt(x, y) }
        onMountDrive: device => { win.info = "Opening the drive…"; Ops.mount(device) }
        // A saved server: connect with its saved password; the dialog if
        // it needs anything more.
        onOpenServer: server => {
            win.info = "Connecting to " + server.name + "…";
            win.serverJob = { id: Ops.connectServer(JSON.stringify(server.rclone ? { rclone: server.rclone }
                                                                                  : server.account ? { account: server.account, address: server.uri }
                                                                                  : { address: server.uri, user: server.user, domain: server.domain })), server: server };
        }
        onConnectServer: connectDialog.show(null)
        onAddCloud: cloudDialog.show()
        onShelfClicked: win.info = shelfBar.items.length === 0 ? "Drop files on Shelf (or Ctrl+Shift+S) to gather them" : ""
        onServerMenu: (server, x, y) => { serverMenu.server = server; serverMenu.openAt(x, y) }
        onEjectDrive: device => { win.info = "Ejecting…"; Ops.eject(device) }
        Menu {
            id: serverMenu
            property var server: ({ name: "", uri: "", path: "", saved: false })
            items: [
                ...(serverMenu.server.path ? [{ label: "Disconnect", run: () => { const e = Ops.disconnectServer(serverMenu.server.uri); if (e) win.notice = e; sidebar.refreshServers() } }]
                                           : [{ label: "Connect", run: () => sidebar.openServer(serverMenu.server) }]),
                { label: "Copy address", run: () => { Ops.setClipboardText(serverMenu.server.uri); win.info = "Copied" } },
                ...(serverMenu.server.saved ? [{ separator: true }, { label: serverMenu.server.rclone ? "Remove this account (the files stay in the cloud)" : "Remove from the sidebar", danger: true, run: () => {
                    if (serverMenu.server.path) Ops.disconnectServer(serverMenu.server.uri);
                    Ops.forgetServer(serverMenu.server.uri);
                    sidebar.refreshServers();
                } }] : []),
            ]
        }
        Menu {
            id: tagMenu
            property string tag
            readonly property bool listed: Session.tagDefs.some(t => t.name === tagMenu.tag)
            items: [
                { label: "Colour", items: Object.keys(Session.palette).map(c => ({
                    label: c.charAt(0).toUpperCase() + c.slice(1), colour: Session.palette[c],
                    checked: (Session.tagDefs.find(t => t.name === tagMenu.tag)?.colour ?? "grey") === c,
                    run: () => {
                        const has = Session.tagDefs.some(t => t.name === tagMenu.tag);
                        Session.setTagDefs(has ? Session.tagDefs.map(t => t.name === tagMenu.tag ? { name: t.name, colour: c } : t)
                                               : Session.tagDefs.concat([{ name: tagMenu.tag, colour: c }]));
                    },
                })) },
                { label: "Rename…", run: () => tagDialog.rename(tagMenu.tag) },
                { separator: true },
                { label: "Take off every file", danger: true, run: () => {
                    const r = JSON.parse(Fs.renameTag(tagMenu.tag, ""));
                    if (!r.ok) win.notice = r.error;
                    win.refreshTags();
                } },
                { label: "Remove from this list", enabled: tagMenu.listed, run: () => Session.setTagDefs(Session.tagDefs.filter(t => t.name !== tagMenu.tag)) },
            ]
        }
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
            // Show / hide the sidebar.
            IconButton { width: 32; height: 32; icon: "panelLeft"; fill: "transparent"; iconColor: Session.sidebar ? Theme.text : Theme.muted; onClicked: Session.setSidebar(!Session.sidebar) }
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
            // (Nothing to show differently on the Start page.)
            visible: !(win.current?.ownView ?? false)
            anchors { right: extraTools.left; rightMargin: 6; verticalCenter: parent.verticalCenter }
            value: win.current?.mode ?? "grid"
            options: [{ icon: "grid", label: "Grid (Ctrl+1)", value: "grid" }, { icon: "viewList", label: "List (Ctrl+2)", value: "list" },
                      { icon: "viewColumns", label: "Columns (Ctrl+3)", value: "columns" },
                      { icon: "viewCards", label: "Cards (Ctrl+4)", value: "cards" }, { icon: "viewUsage", label: "Disk usage (Ctrl+5)", value: "usage" }]
            onPicked: v => { if (win.current) { win.current.setView({ mode: v }); Qt.callLater(() => win.current.focusView()) } }
        }
        // Your own toolbar buttons (Settings › Files › Toolbar & menus).
        Row {
            id: extraTools
            anchors { right: searchButton.left; rightMargin: Session.toolbar.length > 0 ? 6 : 0; verticalCenter: parent.verticalCenter }
            spacing: 2
            Repeater {
                model: Session.toolbar.map(id => Actions.toolbar.find(t => t.id === id)).filter(t => !!t)
                delegate: IconButton {
                    required property var modelData
                    width: 32; height: 32; icon: modelData.icon; fill: "transparent"
                    iconColor: (modelData.id === "hidden" && Session.hidden) || (modelData.id === "details" && Session.details) || (modelData.id === "split" && (win.tab?.split ?? false)) ? Theme.rose : Theme.text
                    onClicked: win.toolbarDo(modelData.id)
                    ToolTip.visible: hovered
                    ToolTip.delay: 500
                    ToolTip.text: modelData.label
                }
            }
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
                // Short, with the rest in submenus.
                items: [
                    { label: "New tab", shortcut: "Ctrl+T", run: () => win.newTab({ path: win.current?.path ?? "" }) },
                    { label: "New window", shortcut: "Ctrl+N", run: () => Session.open({ tabs: [{ path: win.current?.path ?? "" }], current: 0 }) },
                    { separator: true },
                    { label: "Show", items: [
                        { label: "Sidebar", shortcut: "Ctrl+B", checked: Session.sidebar, run: () => Session.setSidebar(!Session.sidebar) },
                        { label: "Details panel", shortcut: "Alt+P", checked: Session.details, run: () => Session.setDetails(!Session.details) },
                        { label: "Hidden files", shortcut: "Ctrl+H", checked: Session.hidden, run: () => Session.setHidden(!Session.hidden) },
                        { label: "Split", shortcut: "F3", checked: win.tab?.split ?? false, run: () => win.tab.setSplit(!win.tab.split) },
                        ...(win.tab?.split ? [{ label: win.tab.vertical ? "Side by side" : "One above the other", run: () => win.tab.setVertical(!win.tab.vertical) }] : []),
                        { separator: true },
                        { label: "Zoom", stepper: { value: Math.round(Session.zoom * 100) + "%",
                                                    minus: () => Session.setZoom(Session.zoom - 0.1), plus: () => Session.setZoom(Session.zoom + 0.1) } },
                        { label: "Pick views to suit each folder", checked: Session.adaptive, run: () => Session.setAdaptive(!Session.adaptive) },
                        { separator: true },
                        { label: "List columns", items: [
                            { label: "Size", checked: Session.columns.includes("size"), run: () => Session.toggleColumn("size") },
                            { label: "Modified", checked: Session.columns.includes("modified"), run: () => Session.toggleColumn("modified") },
                            { label: "Created", checked: Session.columns.includes("created"), run: () => Session.toggleColumn("created") },
                            { label: "Kind", checked: Session.columns.includes("kind"), run: () => Session.toggleColumn("kind") },
                            { label: "Tags", checked: Session.columns.includes("tags"), run: () => Session.toggleColumn("tags") },
                        ] },
                    ] },
                    { label: "Sort by", items: [
                        { label: "Name", checked: win.current?.sortBy === "name", run: () => win.current.setView({ sortBy: "name", descending: false }) },
                        { label: "Size", checked: win.current?.sortBy === "size", run: () => win.current.setView({ sortBy: "size", descending: true }) },
                        { label: "Date modified", checked: win.current?.sortBy === "modified", run: () => win.current.setView({ sortBy: "modified", descending: true }) },
                        { label: "Date created", checked: win.current?.sortBy === "created", run: () => win.current.setView({ sortBy: "created", descending: true }) },
                        { label: "Kind", checked: win.current?.sortBy === "kind", run: () => win.current.setView({ sortBy: "kind", descending: false }) },
                        { label: "Reversed", checked: win.current?.descending ?? false, run: () => win.current.setView({ descending: !win.current.descending }) },
                        { separator: true },
                        { label: "Folders first", checked: Session.foldersAt === "first", run: () => Session.setFoldersAt("first") },
                        { label: "Folders mixed in", checked: Session.foldersAt === "mixed", run: () => Session.setFoldersAt("mixed") },
                        { label: "Folders last", checked: Session.foldersAt === "last", run: () => Session.setFoldersAt("last") },
                    ] },
                    { label: "Group by", items: [
                        { label: "Nothing", checked: (win.current?.group ?? "") === "", run: () => win.current.setView({ group: "" }) },
                        { label: "Kind", checked: win.current?.group === "kind", run: () => win.current.setView({ group: "kind" }) },
                        { label: "Date modified", checked: win.current?.group === "modified", run: () => win.current.setView({ group: "modified" }) },
                        { label: "Size", checked: win.current?.group === "size", run: () => win.current.setView({ group: "size" }) },
                        { label: "Name", checked: win.current?.group === "name", run: () => win.current.setView({ group: "name" }) },
                        { label: "Tag", checked: win.current?.group === "tag", run: () => win.current.setView({ group: "tag" }) },
                    ] },
                    { label: "Workspaces", items: win.workspaceMenu },
                    { label: "Connect to server…", run: () => connectDialog.show(null) },
                    { label: "Add cloud account…", run: () => cloudDialog.show() },
                    { label: "This folder", items: [
                        { label: viewMenu.bookmarked ? "Remove bookmark" : "Bookmark it", shortcut: "Ctrl+D", run: () => win.toggleBookmark() },
                        { label: "Colour and icon…", enabled: !(win.current?.special ?? true), run: () => looks.show(win.current.path, win.current.title) },
                        { label: win.tab?.other && !win.tab.other.special && !win.tab.other.inArchive ? "Compare with the other side…" : "Compare with…",
                          enabled: !(win.current?.special ?? true) && !(win.current?.inArchive ?? false), run: () => win.compare() },
                        { separator: true },
                        { label: "Use its view in subfolders", checked: win.current?.viewInfo.subfolders ?? false,
                          run: () => { Fs.setViewForSubfolders(win.current.path, !win.current.viewInfo.subfolders); win.refreshViews() } },
                        { label: "Use its view everywhere",
                          run: () => { Fs.resetView(win.current.path); Session.setDefaultView({ mode: win.current.mode, sortBy: win.current.sortBy, descending: win.current.descending, group: win.current.group }) } },
                        { label: "Reset its view", enabled: win.current?.viewInfo.own ?? false,
                          run: () => { Fs.resetView(win.current.path); win.refreshViews() } },
                    ] },
                    { separator: true },
                    { label: "Settings…", run: () => Session.openSettings("") },
                    { label: "Keyboard shortcuts…", run: () => keysDialog.open() },
                    { label: "Export settings…", run: () => exportDialog.open() },
                    { label: "Import settings…", run: () => importDialog.open() },
                    { label: "Help", items: [
                        { label: "Copy diagnostics", run: () => {
                            const keys = ["files.startIn", "files.openWith", "files.hidden", "files.git", "files.adaptive", "files.zoom", "files.foldersAt",
                                          "files.columns", "files.defaultView", "files.indexDrives", "files.folderSizes", "apps.terminal", "apps.editor"];
                            const settings = {};
                            for (const k of keys) settings[k] = Session.setting(k, null);
                            Ops.setClipboardText(Fs.diagnostics(JSON.stringify(settings)));
                            win.info = "Diagnostics copied: paste them into a bug report";
                        } },
                        { label: "Open the log", run: () => win.showContents({ path: Fs.logFile() }) },
                    ] },
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
            // Match case, and the text as a regular expression.
            SearchOption { text: "Aa"; tip: "Match case"; on: searchRow.ask.case ?? false; onToggled: v => win.current?.find({ case: v }) }
            SearchOption { text: ".*"; tip: "Regular expression"; on: searchRow.ask.regex ?? false; onToggled: v => win.current?.find({ regex: v }) }
            // Text inside the files (their contents, a PDF's text, or a comment).
            Field {
                id: insideField
                width: 190
                anchors.verticalCenter: parent.verticalCenter
                placeholder: "Text inside files…"
                text: searchRow.ask.content ?? ""
                onAccepted: win.current?.find({ content: text })
                onTextChanged: insideTimer.restart()
                Timer { id: insideTimer; interval: 450; onTriggered: if (insideField.text !== (searchRow.ask.content ?? "")) win.current?.find({ content: insideField.text }) }
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
                onClicked: Search.reindex(JSON.stringify(Session.indexOpts))
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
    // ---- open in a terminal / the code editor -------------------------------------------------
    function choice(key) { try { return JSON.parse(Desktop.setting(key)) ?? "" } catch (e) { return "" } }
    property var editorApp: JSON.parse(Fs.editor(win.choice("apps.editor")))
    function openTerminal(dir) {
        const err = Fs.openTerminal(dir, win.choice("apps.terminal"));
        if (err) win.notice = "Couldn't open a terminal: " + err;
    }
    function openEditor(path) {
        const err = Fs.openEditor(path, win.choice("apps.editor"));
        if (err) win.notice = "Couldn't open the editor: " + err;
    }
    // "Open in terminal" / "Open in VS Code" (and the repository it's in) for
    // a folder (`entry` null: the one you're in) or a text file.
    function openInItems(entry) {
        const p = win.current;
        if (!p || p.inTrash || (p.searching && !entry)) return [];
        const dir = entry ? (entry.dir ? entry.path : "") : p.inDrives ? "" : p.path;
        const textish = entry && !entry.dir && ["Text", "Markdown", "JSON", "Script"].includes(F.kind(entry.mime));
        const ed = win.editorApp;
        const out = [];
        if (dir) out.push({ label: "Open in terminal", shortcut: entry ? "" : "F4", run: () => win.openTerminal(dir) });
        if (ed && (dir || textish)) out.push({ label: "Open in " + ed.name, run: () => win.openEditor(entry ? entry.path : dir) });
        const root = dir ? Fs.gitRoot(dir) : "";
        if (ed && root && root !== dir) out.push({ label: "Open repository in " + ed.name, run: () => win.openEditor(root) });
        return out;
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

    // ---- the selection, previewed (Alt+P) and looked at closely (Space) -------------------------
    Sheet {
        id: details
        // (Nothing to show beside Start or Clean up.)
        visible: Session.details && win.width > 760 && !(win.current?.ownView ?? false)
        anchors { right: parent.right; top: searchRow.bottom; bottom: shelfBar.top }
        width: Math.min(340, Math.max(260, win.width * 0.27))
        pane: win.current
        onRunScript: (dir, command) => {
            const err = Fs.runInTerminal(dir, command, win.choice("apps.terminal"));
            if (err) win.notice = err; else win.info = "Running “" + command + "” in the terminal";
        }
        onTagsEdited: win.refreshTags()
        onRenamed: paths => {
            const p = win.current;
            if (!p) return;
            p.selectPaths(paths);
            p.focusView();
            if (paths.length > 0) win.info = "Renamed " + paths.length + (paths.length === 1 ? " item" : " items");
        }
        onRenameFailed: message => win.notice = message
        onCloseIt: { Session.setDetails(false); win.current?.focusView() }
    }
    QuickLook {
        id: quick
        onStep: by => {
            const p = win.current;
            if (!p || p.shownEntries.length === 0) return;
            const i = Math.max(0, Math.min(p.shownEntries.length - 1, p.current + by));
            p.clickAt(i, false, false);
            quick.entry = p.shownEntries[i];
        }
        onOpenIt: entry => { if (entry) win.current?.activate(entry) }
        onClosed: win.current?.focusView()
    }
    function quickLook() {
        const p = win.current;
        if (!p) return;
        const e = p.selection.length > 0 ? p.selection[0] : p.shownEntries[p.current];
        if (!e) return;
        quick.entry = e;
        quick.open();
    }

    // ---- the folder -----------------------------------------------------------------------
    Item {
        id: stack
        anchors { left: sidebar.right; right: details.visible ? details.left : parent.right; top: searchRow.bottom; bottom: shelfBar.top }

        // Right-click on files (`entry` set) or on the folder's empty space.
        Menu {
            id: itemMenu
            property var entry: null
            readonly property var sel: win.current?.selection ?? []
            readonly property bool many: sel.length > 1
            readonly property bool trashed: win.current?.inTrash ?? false
            readonly property var openIn: win.openInItems(itemMenu.entry)
            // Files of one kind: the apps for it (the default marked).
            // (Only while it's open: it reads the installed apps.)
            readonly property var openWith: itemMenu.visible ? win.openWithItems(itemMenu.sel) : []
            items: win.menuFor(itemMenu.entry, win.current?.inArchive ? (!itemMenu.entry ? [
                { label: "Select all", shortcut: "Ctrl+A", run: () => win.current.selectAll() },
                { label: "Extract everything beside it", run: () => win.run("extract", [win.current.archiveFile], win.current.archiveFile.substring(0, win.current.archiveFile.lastIndexOf("/"))) },
            ] : [
                { label: "Open", shortcut: "Enter", run: () => win.current.activate(itemMenu.entry) },
                { label: "Quick look", shortcut: "Space", enabled: !itemMenu.many && !itemMenu.entry.dir, run: () => win.quickLook() },
                ...(itemMenu.entry.dir && !itemMenu.many ? [{ label: "Open in new tab", run: () => win.newTab({ path: itemMenu.entry.path }, win.currentIndex + 1) }] : []),
                { separator: true },
                { label: "Extract beside the archive", run: () => win.run("copy", itemMenu.sel.map(e => e.path), win.current.archiveFile.substring(0, win.current.archiveFile.lastIndexOf("/"))) },
                ...(win.tab?.other && !win.tab.other.special && !win.tab.other.inArchive ? [{ label: "Extract to the other side", shortcut: "F5", run: () => win.run("copy", itemMenu.sel.map(e => e.path), win.tab.other.path) }] : []),
            ]) : win.current?.inDrives ? (!itemMenu.entry ? [{ label: "Clean up…", run: () => win.cleanUp("") }] : [
                { label: "Open", shortcut: "Enter", run: () => win.current.activate(itemMenu.entry) },
                { label: "Open in new tab", run: () => win.newTab({ path: itemMenu.entry.path }, win.currentIndex + 1) },
                { label: "Clean up…", run: () => win.cleanUp(itemMenu.entry.path) },
                ...itemMenu.openIn,
                { separator: true },
                { label: "Properties", shortcut: "Alt+Enter", run: () => win.properties() },
            ]) : (win.current?.inCollection && !itemMenu.entry) ? [
                { label: "Select all", shortcut: "Ctrl+A", run: () => win.current.selectAll() },
                ...(win.current.inTag ? [{ label: "Rename tag…", run: () => tagDialog.rename(win.current.path.slice(4)) }] : []),
            ] : itemMenu.trashed ? (!itemMenu.entry ? [
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
                ...(itemMenu.openIn.length > 0 ? [{ separator: true }].concat(itemMenu.openIn) : []),
                { separator: true },
                { label: (Session.hidden ? "Hide" : "Show") + " hidden files", shortcut: "Ctrl+H", run: () => Session.setHidden(!Session.hidden) },
                { label: "Copy this folder's path", run: () => Ops.setClipboardText(win.current.path) },
                { label: "Find what takes room here…", run: () => win.cleanUp(win.current.path) },
                { label: "Colour and icon…", run: () => looks.show(win.current.path, win.current.title) },
                { label: "Properties", run: () => { win.current.selected = {}; win.properties() } },
            ] : [
                { label: "Open", shortcut: "Enter", run: () => win.current.activate(itemMenu.entry) },
                { label: "Quick look", shortcut: "Space", enabled: !itemMenu.many, run: () => win.quickLook() },
                ...(win.current?.searching || win.current?.inCollection ? [{ label: "Show in folder", run: () => win.showInFolder(itemMenu.entry) }] : []),
                ...(itemMenu.entry.dir && !itemMenu.many ? [{ label: "Open in new tab", run: () => win.newTab({ path: itemMenu.entry.path }, win.currentIndex + 1) }] : []),
                ...(itemMenu.openWith.length > 1 ? [{ label: "Open with", items: itemMenu.openWith }] : []),
                ...(itemMenu.many ? [] : itemMenu.openIn),
                { separator: true },
                { label: "Cut", shortcut: "Ctrl+X", run: () => win.toClipboard(true) },
                { label: "Copy", shortcut: "Ctrl+C", run: () => win.toClipboard(false) },
                ...(itemMenu.entry.dir && !itemMenu.many ? [{ label: "Paste into folder", enabled: win.clipFiles.length > 0, run: () => win.paste(itemMenu.entry.path) }] : []),
                { label: "Duplicate", run: () => win.run("copy", itemMenu.sel.map(e => e.path), win.current.path) },
                { label: "Copy path", items: [
                    { label: itemMenu.many ? "Paths" : "Path", run: () => win.copyPaths("plain") },
                    { label: "Quoted, for a terminal", run: () => win.copyPaths("quoted") },
                    { label: "As links (file://)", run: () => win.copyPaths("uri") },
                    { label: itemMenu.many ? "Names" : "Name", run: () => win.copyPaths("name") },
                ] },
                ...(itemMenu.sel.length > 0 && itemMenu.sel.every(e => !e.dir && Fs.isArchive(e.name)) && !win.current?.inTrash
                    ? [{ label: "Extract here", run: () => win.run("extract", itemMenu.sel.map(e => e.path), win.current.path) }] : []),
                ...(win.current?.inTrash || win.current?.inCollection || win.current?.searching ? [] : [{ label: "Compress…", run: () => compressDialog.show(itemMenu.sel.map(e => e.path), win.current.path) }]),
                ...(win.tab?.other ? [
                    { label: "Copy to other side", shortcut: "F5", run: () => win.toOther("copy") },
                    { label: "Move to other side", shortcut: "F6", run: () => win.toOther("move") },
                ] : []),
                { separator: true },
                { label: itemMenu.many ? "Rename " + itemMenu.sel.length + " items…" : "Rename…", shortcut: "F2", run: () => win.askName("rename") },
                ...(win.current?.searching || win.current?.inCollection ? [] : [{ label: "New folder with selection…", run: () => win.askName("group") }]),
                ...(itemMenu.entry.dir && !itemMenu.many ? [{ label: "Colour and icon…", run: () => looks.show(itemMenu.entry.path, itemMenu.entry.name) }] : []),
                ...(win.current?.inArchive ? [] : [{ label: "Put on the shelf", shortcut: "Ctrl+Shift+S", run: () => win.toShelf() }]),
                { label: "Tags", items: win.tagItems(itemMenu.sel.map(e => e.path)) },
                // In a folder a sync client keeps (Nextcloud, ownCloud).
                ...(win.current?.cloud.client ? [{ label: win.current.cloud.client, items: [
                    { label: "Keep on this device", run: () => win.cloudAct("local") },
                    { label: "Free up space (keep only in the cloud)", run: () => win.cloudAct("online") },
                    { separator: true },
                    { label: "Share…", enabled: !itemMenu.many, run: () => win.cloudAct("share") },
                    { label: "Copy link", enabled: !itemMenu.many, run: () => win.cloudAct("link") },
                ] }] : []),
                { label: "Move to Trash", shortcut: "Delete", run: () => win.trash() },
                { label: "Delete permanently…", shortcut: "Shift+Del", danger: true, run: () => win.askDelete(itemMenu.sel.map(e => e.path)) },
                { separator: true },
                { label: "Properties", shortcut: "Alt+Enter", run: () => win.properties() },
            ])
        }

        Transfers {
            id: transfers
            anchors { right: parent.right; bottom: parent.bottom; margins: 14 }
            z: 20
            onDeleteInstead: paths => win.askDelete(paths)
        }
        // Copies and moves that didn't finish last time (Files closed, or
        // the computer went off part way): carry on, or forget them.
        Column {
            anchors { right: parent.right; bottom: transfers.top; margins: 14; bottomMargin: 8 }
            width: 360
            spacing: 8
            z: 21
            Repeater {
                model: win.unfinished
                delegate: Rectangle {
                    id: uf
                    required property var modelData
                    width: 360; height: ufCol.implicitHeight + 28
                    radius: 14
                    color: Theme.surface1
                    border { width: 1; color: Theme.surface2 }
                    Column {
                        id: ufCol
                        x: 14; y: 14; width: parent.width - 28
                        spacing: 8
                        Text {
                            width: parent.width
                            wrapMode: Text.Wrap
                            text: (uf.modelData.kind === "move" ? "Moving " : "Copying ") + uf.modelData.remaining.length + (uf.modelData.remaining.length === 1 ? " item" : " items")
                                + " to " + uf.modelData.dest.substring(uf.modelData.dest.lastIndexOf("/") + 1) + " didn't finish"
                            color: Theme.text
                            font { family: Theme.font; pointSize: Theme.pt(12); weight: Font.Medium }
                        }
                        Text {
                            width: parent.width
                            wrapMode: Text.Wrap
                            text: "Started " + F.date(uf.modelData.started * 1000, Date.now()) + ". Carrying on skips what's already there and finishes the rest."
                            color: Theme.muted
                            font { family: Theme.font; pointSize: Theme.pt(11) }
                        }
                        Row {
                            anchors.right: parent.right
                            spacing: 6
                            Button { text: "Forget it"; kind: "ghost"; onClicked: { Ops.forgetTransfer(uf.modelData.key); win.unfinished = win.unfinished.filter(x => x.key !== uf.modelData.key) } }
                            Button {
                                text: "Carry on"; kind: "primary"
                                onClicked: {
                                    const id = Ops.resumeTransfer(uf.modelData.key);
                                    if (id < 0) win.info = "Nothing was left to do";
                                    win.unfinished = win.unfinished.filter(x => x.key !== uf.modelData.key);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // ---- the shelf ------------------------------------------------------------------------
    ShelfBar {
        id: shelfBar
        anchors { left: sidebar.right; right: parent.right; bottom: status.top }
        here: (win.current && win.current.path.startsWith("/") && !win.current.readOnly) ? win.current.path : ""
        onItemsChanged: sidebar.shelfCount = items.length > 0 ? items.length : -1
        onSaid: text => win.info = text
        onCopyHere: paths => win.run("copy", paths, win.current.path)
        onMoveHere: paths => win.run("move", paths, win.current.path)
    }
    function toShelf() {
        const sel = win.current?.selection ?? [];
        if (sel.length > 0 && !win.current.inArchive) shelfBar.add(sel.map(e => e.path));
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
                if (win.info !== "") return win.info;
                const p = win.current;
                if (!p || p.ownView) return "";
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
        // The git branch of the repository this is in, and how far ahead of
        // / behind its remote (just shown: git itself is for the terminal).
        Row {
            id: branchRow
            readonly property var git: win.current?.git ?? ({ root: "" })
            visible: git.root !== "" && git.branch !== ""
            anchors { right: parent.right; rightMargin: 16; verticalCenter: parent.verticalCenter }
            spacing: 5
            Icon { anchors.verticalCenter: parent.verticalCenter; name: "branch"; size: 12; color: Theme.rose }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: parent.git.branch ?? ""
                color: Theme.muted
                font { family: Theme.mono; pointSize: Theme.pt(10.5) }
            }
            Text {
                visible: (parent.git.ahead ?? 0) > 0 || (parent.git.behind ?? 0) > 0
                anchors.verticalCenter: parent.verticalCenter
                text: ((parent.git.ahead ?? 0) > 0 ? "↑" + parent.git.ahead : "") + ((parent.git.ahead ?? 0) > 0 && (parent.git.behind ?? 0) > 0 ? " " : "") + ((parent.git.behind ?? 0) > 0 ? "↓" + parent.git.behind : "")
                color: Theme.faint
                font { family: Theme.mono; pointSize: Theme.pt(10.5) }
            }
        }
    }

    // ---- file operations ---------------------------------------------------------------------
    // Files on the clipboard (checked when a menu opens or Paste is pressed).
    property var clipFiles: []
    property bool clipCut: false
    // Jobs started here, and the tab that wants their results selected.
    property var waiting: ({})
    readonly property bool typing: tagDialog.visible || keysDialog.visible || looks.visible || address.editing || nameDialog.visible || confirm.visible || details.typing || searchField.editing || quick.visible

    function readClipboard() {
        const c = JSON.parse(Ops.clipboard());
        win.clipFiles = c.paths;
        win.clipCut = c.cut;
    }
    function showMenu(entry, x, y) {
        win.readClipboard();
        if (entry) win.menuTags = JSON.parse(Fs.tagsOn(JSON.stringify((win.current?.selection ?? []).map(e => e.path))));
        itemMenu.entry = entry;
        // (The Storage page's empty space has nothing to offer.)
        if (!entry && win.current?.inDrives) return;
        Qt.callLater(() => itemMenu.openAt(Math.min(x, stack.width - itemMenu.width - 8), Math.min(y, stack.height - itemMenu.height - 8)));
    }
    function run(kind, paths, dest) {
        if (paths.length === 0) return;
        const id = Ops.start(kind, JSON.stringify(paths), dest ?? "");
        if (kind === "extract" && id >= 0) { const e = Object.assign({}, win.extracting); e[id] = dest ?? ""; win.extracting = e }
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
        // On the shelf: gathered there.
        if (dest === "shelf:") { shelfBar.add(paths); return }
        // On a tag: tagged with it.
        if (dest.startsWith("tag:")) { win.tag(paths, dest.slice(4), true); return }
        const mods = Ops.modifiers();
        const plan = Drops.plan(paths, dest, (mods & Qt.ControlModifier) !== 0, (mods & Qt.ShiftModifier) !== 0,
                                Fs.sameDrive(paths[0], dest));
        if (plan) win.run(plan.kind, plan.paths, plan.kind === "trash" ? "" : dest);
    }
    // ---- tags ---------------------------------------------------------------------------
    // The tags the right-clicked selection has ({tag: how many have it}).
    property var menuTags: ({})
    function tag(paths, name, on) {
        const r = JSON.parse(Fs.setTag(JSON.stringify(paths), name, on));
        if (r.failed.length > 0) win.notice = "Couldn't tag “" + r.failed[0][0].substring(r.failed[0][0].lastIndexOf("/") + 1) + "”: " + r.failed[0][1];
        if (on) Session.defineTag(name, Session.freshColour());
        win.refreshTags();
    }
    function refreshTags() {
        for (const w of Session.windows) { for (const t of w.tabs) for (const p of t.panes) p.load(); w.refreshSidebar() }
    }
    // The Tags submenu for these paths: every tag, ticked when they all have it.
    function tagItems(paths) {
        const on = win.menuTags;
        const names = Session.tagDefs.map(t => t.name).concat(Object.keys(on).filter(t => !Session.tagDefs.some(d => d.name === t)));
        return names.map(n => ({
            label: n, colour: Session.tagColour(n), checked: (on[n] ?? 0) === paths.length,
            run: () => win.tag(paths, n, (on[n] ?? 0) !== paths.length),
        })).concat(names.length > 0 ? [{ separator: true }] : [], [{ label: "New tag…", run: () => tagDialog.newTag(paths) }]);
    }
    TagDialog { id: tagDialog; onDone: win.refreshTags(); onClosed: win.current?.focusView() }

    // The selection to the folder on the other side of a split tab.
    // Compare & sync: this folder and the other side's (or one to choose).
    function compare() {
        const o = win.tab?.other;
        compareDialog.show_(win.current.path, o && !o.special && !o.inArchive ? o.path : "");
    }
    function toOther(kind) {
        const sel = win.current?.selection ?? [];
        const other = win.tab?.other;
        if (sel.length === 0 || !other || other.special || win.current.inDrives) return;
        win.run(kind, sel.map(e => e.path), other.path);
    }
    // Clean up, looking under `root` (home if "").
    function cleanUp(root) { Session.cleanupRoot = root ?? ""; win.current?.go("cleanup:") }
    function openWithItems(sel) {
        if (!sel || sel.length === 0 || sel.some(e => e.dir) || sel.some(e => e.mime !== sel[0].mime)) return [];
        const paths = sel.map(e => e.path);
        return JSON.parse(Fs.appsFor(sel[0].mime)).slice(0, 12).map(a => ({
            label: a.name + (a.default ? "  (default)" : ""),
            run: () => { const err = Fs.openWith(a.id, JSON.stringify(paths)); if (err) win.notice = err }
        }));
    }
    // The selection's paths as text: plain, quoted for a shell, file:// links, or names.
    // Asks the sync client (Nextcloud, ownCloud) to act on the selection.
    function cloudAct(what) {
        const sel = win.current?.selection ?? [];
        if (sel.length === 0) return;
        const err = Ops.cloudAct(what, JSON.stringify(sel.map(e => e.path)));
        if (err) { win.notice = err; return }
        win.info = ({ local: "Downloading to keep on this device…", online: "Freeing up space…", share: "", link: "Link copied" })[what];
        Qt.callLater(() => win.current?.askCloud());
    }
    function copyPaths(how) {
        const sel = win.current?.selection ?? [];
        if (sel.length === 0) return;
        Ops.setClipboardText(F.pathsAs(sel.map(e => e.path), how));
        win.info = sel.length === 1 ? "Copied" : "Copied " + sel.length + (how === "name" ? " names" : " paths");
    }
    function toClipboard(cut) {
        // (Drives aren't files to copy or move.)
        if (win.current?.inDrives) return;
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
        if (win.current?.inDrives) return;
        if (win.current?.inTrash) win.askDelete(sel.map(e => e.path), "purge");
        // (Settings › Files › General: ask first.)
        else if (Session.confirmTrash) win.askDelete(sel.map(e => e.path), "trash");
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
        if (sel.length === 0 && win.current?.special) return;
        // In the Sheet, beside the files.
        details.tab = "info";
        Session.setDetails(true);
        details.refresh();
    }
    // A line for the status bar, for a few seconds (not a problem: not logged).
    property string info: ""
    Timer { running: win.info !== ""; interval: 5000; onTriggered: win.info = "" }
    // A line for the status bar, for a few seconds.
    property string notice: ""
    // (Notices are problems: they go in the log too.)
    onNoticeChanged: if (win.notice !== "") Fs.log("warn", win.notice)

    // ---- drives: mounted (then opened) and ejected ----------------------------------------
    Connections {
        target: Ops
        function onDriveDone(result) {
            const r = JSON.parse(result);
            sidebar.refresh();
            win.info = "";
            if (r.error) { win.notice = (r.action === "mount" ? "Couldn't open the drive: " : "Couldn't eject it: ") + r.error; return }
            if (r.action === "mount") { win.current?.go(r.path); win.current?.focusView(); return }
            win.info = "You can unplug it now";
        }
    }

    // ---- opening files: programs ask first ---------------------------------------------
    function openFile(entry) {
        // Inside an archive: taken out into a cache first (an archive in
        // an archive opens as a folder too). Programs from there count as
        // untrusted (run.rs).
        if (entry.path.startsWith("archive:")) {
            const real = Fs.archiveFile(entry.path);
            if (!real) { win.notice = "Couldn't take “" + entry.name + "” out of the archive"; return }
            if (Fs.isArchive(entry.name)) { win.current?.go("archive:" + real + "!/"); return }
            entry = Object.assign({}, entry, { path: real });
        } else if (Fs.isArchive(entry.name)) {
            // An archive opens like a folder (Extract here is in the menu).
            win.current?.go("archive:" + entry.path + "!/");
            return;
        }
        const info = JSON.parse(Fs.runInfo(entry.path, entry.mime ?? ""));
        if (info.kind === "none") { Qt.openUrlExternally("file://" + entry.path); return }
        runDialog.ask(entry, info);
    }
    // As text, in Notepad (or the code editor).
    function showContents(entry) {
        const err = Fs.openEditor(entry.path, "whiterose-notepad");
        if (err) win.notice = "Couldn't show it: " + err;
    }
    RunDialog {
        id: runDialog
        onRun: entry => { const err = Fs.runProgram(entry.path); if (err) win.notice = "Couldn't run it: " + err }
        onShowContents: entry => win.showContents(entry)
        onClosed: win.current?.focusView()
    }
    Timer { running: win.notice !== ""; interval: 6000; onTriggered: win.notice = "" }
    // "rename", "folder" or "file".
    function askName(what) {
        const p = win.current;
        if (!p) return;
        // Several selected: rename them all at once.
        if (what === "rename" && p.selection.length > 1) { details.tab = "rename"; Session.setDetails(true); return }
        const e = what === "rename" ? p.selection[0] : null;
        if (what === "rename" && !e) return;
        nameDialog.what = what;
        nameDialog.entry = e;
        nameDialog.error = "";
        nameField.text = e ? e.name : what === "folder" ? "New folder"
            : what === "group" ? F.groupName(p.selection.map(x => x.name)) : "New file";
        // (A folder made from the selection: what goes in it.)
        nameDialog.group = what === "group" ? p.selection.map(x => x.path) : [];
        nameDialog.open();
    }
    function nameChosen() {
        const p = win.current, name = nameField.text;
        if (nameDialog.what === "workspace" || nameDialog.what === "renameWorkspace") {
            const err = nameDialog.what === "workspace" ? Session.saveWorkspace(name) : Fs.renameWorkspace(nameDialog.entry.name, name);
            if (err !== "") { nameDialog.error = err; return }
            Session.workspacesChanged();
            nameDialog.close();
            win.info = nameDialog.what === "workspace" ? "Saved as “" + name.trim() + "”" : "Renamed";
            return;
        }
        const r = JSON.parse(nameDialog.what === "rename" ? Ops.rename(nameDialog.entry.path, name)
                                                          : Ops.create(p.path, name, nameDialog.what === "file"));
        if (!r.ok) { nameDialog.error = r.error; return }
        nameDialog.close();
        if (nameDialog.what === "group" && nameDialog.group.length > 0) {
            // The selection goes into the new folder.
            win.run("move", nameDialog.group, r.path);
        }
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
    // Extracting needed a password (or the one given was wrong): ask, and try again.
    Connections {
        target: Ops
        function onFinished(id, result) {
            const r = JSON.parse(result);
            if (r.kind !== "extract" || win.extracting[id] === undefined) return;
            const dest = win.extracting[id];
            const e = Object.assign({}, win.extracting);
            delete e[id];
            win.extracting = e;
            const asked = r.failed.find(f => /protected with a password|password isn't right/.test(f[1]));
            if (asked) archivePassword.ask(asked[0], dest, /isn't right/.test(asked[1]));
        }
    }
    // Extract jobs running (id: where to), for asking a password and trying again.
    property var extracting: ({})
    // ---- toolbar & menus (Settings › Files › Toolbar & menus) ---------------------------
    // The right-click menu as set up: what's left out goes, and your own
    // actions that fit what's chosen come after "Open".
    function menuFor(entry, items) {
        let out = Actions.trim(items, Session.menuHide);
        const sel = win.current?.selection ?? [];
        if (entry && !win.current?.special && !win.current?.inArchive) {
            const mine = Session.actions.filter(a => Actions.applies(a, sel)).map(a => ({ label: a.name, run: () => win.runAction(a, sel.map(e => e.path)) }));
            if (mine.length > 0) {
                const at = out.findIndex(x => x.separator);
                out = at < 0 ? out.concat([{ separator: true }], mine) : out.slice(0, at).concat([{ separator: true }], mine, out.slice(at));
            }
        }
        return out;
    }
    function runAction(a, paths) {
        const dir = win.current.path;
        const cmd = Actions.command(a.command, paths, dir);
        const err = a.terminal ? Fs.runInTerminal(dir, cmd, win.choice("apps.terminal")) : Fs.runCommand(dir, cmd);
        if (err) win.notice = err; else win.info = "Ran “" + a.name + "”";
    }
    // A toolbar button pressed.
    function toolbarDo(id) {
        const p = win.current;
        if (!p) return;
        const sel = p.selection;
        switch (id) {
        case "new-folder": if (!p.readOnly && !p.special) win.askName("folder"); break;
        case "refresh": p.load(); break;
        case "terminal": if (p.path.startsWith("/")) win.openTerminal(p.path); break;
        case "editor": if (p.path.startsWith("/")) win.openEditor(p.path); break;
        case "split": win.tab.setSplit(!win.tab.split); break;
        case "hidden": Session.setHidden(!Session.hidden); break;
        case "details": Session.setDetails(!Session.details); break;
        case "shelf": win.toShelf(); break;
        case "compress": if (sel.length > 0 && !p.special) compressDialog.show(sel.map(e => e.path), p.path); break;
        case "compare": if (!p.special && !p.inArchive) win.compare(); break;
        case "new-tab": win.newTab({ path: p.path }); break;
        }
    }

    // ---- reliability ---------------------------------------------------------------
    // Interrupted transfers (looked at once, shortly after starting).
    property var unfinished: []
    Timer { running: true; interval: 1200; onTriggered: win.unfinished = JSON.parse(Ops.interrupted()) }
    FileDialog {
        id: exportDialog
        title: "Export Files' settings"
        fileMode: FileDialog.SaveFile
        currentFolder: "file://" + Fs.home()
        selectedFile: "file://" + Fs.home() + "/whiterose-files-settings.json"
        nameFilters: ["Files settings (*.json)"]
        onAccepted: {
            const err = Ops.exportSettings(decodeURIComponent(String(selectedFile).replace(/^file:\/\//, "")));
            if (err) win.notice = err; else win.info = "Settings exported (no passwords: those stay in your keyring)";
        }
    }
    FileDialog {
        id: importDialog
        title: "Import Files' settings"
        currentFolder: "file://" + Fs.home()
        nameFilters: ["Files settings (*.json)"]
        onAccepted: {
            const r = JSON.parse(Ops.importSettings(decodeURIComponent(String(selectedFile).replace(/^file:\/\//, ""))));
            if (!r.ok) { win.notice = r.error; return }
            if (r.failed.length > 0) win.notice = "Imported, but " + r.failed.length + " settings weren't taken: " + r.failed[0];
            else win.info = "Imported " + r.settings + " settings and " + r.files + " lists (what you had is kept in before-import.json)";
            win.refreshViews();
            sidebar.refresh();
            sidebar.refreshServers();
        }
    }

    // ---- network servers -------------------------------------------------------
    property var serverJob: null
    Connections {
        target: Ops
        function onServerConnected(id, result) {
            if (!win.serverJob || id !== win.serverJob.id) return;
            const r = JSON.parse(result), server = win.serverJob.server;
            win.serverJob = null;
            win.info = "";
            sidebar.refreshServers();
            if (r.result === "ok") { win.current?.go(r.path); win.current?.focusView() }
            else if (server.rclone) win.notice = "Couldn't open " + server.name + ": " + r.error
            else if (server.account) win.notice = "Couldn't open " + server.name + ": " + (r.error ?? "it needs signing in again in Settings › Online Accounts")
            else { connectDialog.show(server); connectDialog.error = r.result === "failed" ? r.error : r.result === "question" ? "" : "This server needs a password"; connectDialog.question = r.result === "question" ? r.text : "" }
        }
    }
    CloudAccountDialog {
        id: cloudDialog
        // Added: shown in the sidebar, and opened.
        onAdded: name => { sidebar.refreshServers(); sidebar.openServer({ name: name, rclone: name, uri: "rclone:" + name }) }
    }
    ConnectDialog {
        id: connectDialog
        onChanged: sidebar.refreshServers()
        onOpened_: path => { win.current?.go(path); win.current?.focusView() }
    }
    // (A sync shows in Transfers like any copy.)
    CompareDialog { id: compareDialog }
    CompressDialog {
        id: compressDialog
        onMade: id => {
            const shows = (win.tab?.panes ?? []).find(p => p.path === compressDialog.dir);
            if (shows) { const w = Object.assign({}, win.waiting); w[id] = shows; win.waiting = w }
        }
    }
    ArchivePassword { id: archivePassword; onRetry: (archive, dest) => win.run("extract", [archive], dest) }

    // Name for a rename, a new folder or a new file.
    Popup {
        id: nameDialog
        property string what: "rename"
        property var entry: null
        property var group: []
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
                text: nameDialog.what === "rename" ? "Rename" : nameDialog.what === "folder" ? "New folder"
                    : nameDialog.what === "group" ? "New folder with " + nameDialog.group.length + (nameDialog.group.length === 1 ? " item" : " items")
                    : nameDialog.what === "workspace" ? "Save as a workspace" : nameDialog.what === "renameWorkspace" ? "Rename workspace" : "New file"
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
                    : confirm.kind === "trash" ? (confirm.names.length === 1 ? "Move “" + confirm.names[0] + "” to the Trash?" : "Move " + confirm.paths.length + " items to the Trash?")
                    : confirm.names.length === 1 ? "Delete “" + confirm.names[0] + "” permanently?"
                    : "Delete " + confirm.paths.length + " items permanently?"
                wrapMode: Text.Wrap
                color: Theme.text
                font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
            }
            Text {
                width: 340
                text: confirm.kind === "trash" ? "You can bring them back from the Trash."
                    : confirm.kind === "delete" ? "They won't go to the Trash, so they can't be brought back."
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
                    text: confirm.kind === "empty" ? "Empty Trash" : confirm.kind === "trash" ? "Move to Trash" : "Delete"
                    kind: confirm.kind === "trash" ? "primary" : "danger"
                    // After the key is handled: closing in the middle of it crashes Qt.
                    Keys.onReturnPressed: e => { e.accepted = true; Qt.callLater(() => deleteButton.clicked()) }
                    Keys.onEscapePressed: confirm.close()
                    onClicked: { win.run(confirm.kind, confirm.paths); confirm.close() }
                }
            }
        }
    }

    // ---- keys -------------------------------------------------------------------------------
    // Every shortcut, with its default; files.keys ({id: "Ctrl+…"}, set in
    // the Keyboard shortcuts dialog) changes any of them.
    readonly property var keyList: FilesKeys.list
    property var keyOverrides: win.readKeys()
    function readKeys() { try { return JSON.parse(Desktop.setting("files.keys")) ?? {} } catch (e) { return {} } }
    function keys(id, fallback) { return win.keyOverrides[id] ?? fallback }
    Connections {
        target: Desktop
        function onSettingChanged(key) {
            if (key === "files.keys") win.keyOverrides = win.readKeys();
            if (key === "apps.editor") win.editorApp = JSON.parse(Fs.editor(win.choice("apps.editor")));
        }
    }
    // A folder's colour and icon: every tab, window and the sidebar show the change.
    LookDialog {
        id: looks
        onChanged: { for (const w of Session.windows) { for (const t of w.tabs) for (const p of t.panes) p.load(); w.refreshSidebar() } }
        onClosed: win.current?.focusView()
    }
    function refreshSidebar() { sidebar.refresh() }
    KeysDialog { id: keysDialog; list: win.keyList; overrides: win.keyOverrides; onClosed: win.current?.focusView() }
    Shortcut { sequence: win.keys("new-tab", "Ctrl+T"); onActivated: win.newTab({ path: win.current?.path ?? "" }) }
    Shortcut { sequence: win.keys("reopen-tab", "Ctrl+Shift+T"); onActivated: win.reopenTab() }
    Shortcut { sequence: win.keys("next-tab", "Ctrl+PgDown"); onActivated: win.show((win.currentIndex + 1) % win.tabs.length) }
    Shortcut { sequence: win.keys("previous-tab", "Ctrl+PgUp"); onActivated: win.show((win.currentIndex - 1 + win.tabs.length) % win.tabs.length) }
    Shortcut { sequence: win.keys("move-tab-right", "Ctrl+Shift+PgDown"); onActivated: { if (win.currentIndex < win.tabs.length - 1) win.moveTab(win.currentIndex, win.currentIndex + 1) } }
    Shortcut { sequence: win.keys("move-tab-left", "Ctrl+Shift+PgUp"); onActivated: { if (win.currentIndex > 0) win.moveTab(win.currentIndex, win.currentIndex - 1) } }
    Shortcut { sequence: win.keys("new-window", "Ctrl+N"); onActivated: Session.open({ tabs: [{ path: win.current?.path ?? "" }], current: 0 }) }
    Shortcut { sequence: win.keys("close-tab", "Ctrl+W"); onActivated: win.closeTab(win.currentIndex) }
    Shortcut { sequence: win.keys("next-tab-2", "Ctrl+Tab"); onActivated: win.show((win.currentIndex + 1) % win.tabs.length) }
    Shortcut { sequence: win.keys("previous-tab-2", "Ctrl+Shift+Tab"); onActivated: win.show((win.currentIndex - 1 + win.tabs.length) % win.tabs.length) }
    Shortcut { sequence: win.keys("type-path", "Ctrl+L"); onActivated: address.edit() }
    Shortcut { sequence: win.keys("jump", "Ctrl+J"); onActivated: address.jump() }
    Shortcut { sequence: win.keys("back", "Alt+Left"); onActivated: win.current?.goBack() }
    Shortcut { sequence: win.keys("forward", "Alt+Right"); onActivated: win.current?.goForward() }
    // (Backspace is the type-to-filter's while it has letters.)
    Shortcut { sequence: win.keys("up", "Alt+Up"); enabled: !win.typing; onActivated: win.current?.up() }
    Shortcut { sequence: win.keys("up-2", "Backspace"); enabled: !win.typing && (win.current?.filter ?? "") === "" && !(win.current?.searching ?? false); onActivated: win.current?.up() }
    Shortcut { sequence: win.keys("search", "Ctrl+F"); onActivated: win.openSearch(win.current?.filter ?? "") }
    // (Space is the type-to-filter's once it has letters.)
    Shortcut { sequence: win.keys("quick-look", "Space"); enabled: !win.typing && (win.current?.filter ?? "") === ""; onActivated: win.quickLook() }
    Shortcut { sequence: win.keys("details", "Alt+P"); onActivated: Session.setDetails(!Session.details) }
    Shortcut { sequence: win.keys("sidebar", "Ctrl+B"); onActivated: Session.setSidebar(!Session.sidebar) }
    Shortcut { sequence: win.keys("terminal", "F4"); enabled: !win.typing && !!win.current && !win.current.special; onActivated: win.openTerminal(win.current.path) }
    Shortcut { sequence: win.keys("grid", "Ctrl+1"); onActivated: win.current?.setView({ mode: "grid" }) }
    Shortcut { sequence: win.keys("list", "Ctrl+2"); onActivated: win.current?.setView({ mode: "list" }) }
    Shortcut { sequence: win.keys("columns", "Ctrl+3"); onActivated: win.current?.setView({ mode: "columns" }) }
    Shortcut { sequence: win.keys("cards", "Ctrl+4"); onActivated: win.current?.setView({ mode: "cards" }) }
    Shortcut { sequence: win.keys("usage", "Ctrl+5"); onActivated: win.current?.setView({ mode: "usage" }) }
    Shortcut { sequences: [win.keys("bigger", "Ctrl+="), "Ctrl++"]; onActivated: Session.setZoom(Session.zoom + 0.1) }
    Shortcut { sequence: win.keys("smaller", "Ctrl+-"); onActivated: Session.setZoom(Session.zoom - 0.1) }
    Shortcut { sequence: win.keys("normal-size", "Ctrl+0"); onActivated: Session.setZoom(1) }
    Shortcut { sequence: win.keys("hidden", "Ctrl+H"); onActivated: Session.setHidden(!Session.hidden) }
    Shortcut { sequence: win.keys("select-all", "Ctrl+A"); enabled: !win.typing; onActivated: win.current?.selectAll() }
    Shortcut { sequence: win.keys("bookmark", "Ctrl+D"); onActivated: win.toggleBookmark() }
    // F5 / F6 copy / move to the other side of a split tab (reload when there's none).
    Shortcut { sequence: win.keys("copy-other", "F5"); enabled: !win.typing; onActivated: { if (win.tab?.other) win.toOther("copy"); else win.current?.load() } }
    Shortcut { sequence: win.keys("move-other", "F6"); enabled: !win.typing && !!win.tab?.other && !(win.current?.inArchive ?? false); onActivated: win.toOther("move") }
    Shortcut { sequence: win.keys("split", "F3"); enabled: !win.typing; onActivated: win.tab?.setSplit(!win.tab.split) }
    // Tab: the other side.
    Shortcut { sequence: win.keys("other-side", "Tab"); enabled: !win.typing && !!win.tab?.other; onActivated: { win.tab.active = 1 - win.tab.active; win.tab.focusView() } }
    Shortcut { sequence: win.keys("reload", "Ctrl+R"); onActivated: win.current?.load() }
    Shortcut { sequence: win.keys("copy", "Ctrl+C"); enabled: !win.typing && !(win.current?.inArchive ?? false); onActivated: win.toClipboard(false) }
    Shortcut { sequence: win.keys("cut", "Ctrl+X"); enabled: !win.typing && !(win.current?.inArchive ?? false); onActivated: win.toClipboard(true) }
    Shortcut { sequence: win.keys("paste", "Ctrl+V"); enabled: !win.typing && !(win.current?.special ?? false) && !(win.current?.readOnly ?? false); onActivated: { if (win.current) win.paste(win.current.path) } }
    Shortcut { sequence: win.keys("trash", "Delete"); enabled: !win.typing && !(win.current?.inArchive ?? false); onActivated: win.trash() }
    Shortcut { sequence: win.keys("delete", "Shift+Del"); enabled: !win.typing && !(win.current?.inDrives ?? false) && !(win.current?.inArchive ?? false); onActivated: win.askDelete((win.current?.selection ?? []).map(e => e.path), win.current?.inTrash ? "purge" : "delete") }
    Shortcut { sequence: win.keys("shelf", "Ctrl+Shift+S"); enabled: !win.typing; onActivated: win.toShelf() }
    Shortcut { sequence: win.keys("rename", "F2"); enabled: !win.typing && !(win.current?.special ?? false) && !(win.current?.inArchive ?? false); onActivated: win.askName("rename") }
    Shortcut { sequences: [win.keys("properties", "Alt+Return"), "Alt+Enter"]; enabled: !win.typing; onActivated: win.properties() }
    Shortcut { sequence: win.keys("new-folder", "Ctrl+Shift+N"); enabled: !win.typing && !(win.current?.special ?? false) && !(win.current?.readOnly ?? false); onActivated: win.askName("folder") }
}
