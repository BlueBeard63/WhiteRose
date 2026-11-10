// One tab: a folder, its history (back / forward / up), its entries in the
// chosen view (grid, list or columns), and the selection. The listing
// refreshes by itself when files are added, removed or renamed.
import QtQuick
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F
import "Layout.js" as Layout
import "Arrange.js" as Arrange

FocusScope {
    id: pane
    property string path: Fs.home()
    property var back: []
    property var forward: []
    property var entries: []
    property string error: ""
    property real stamp: 0
    property var selected: ({})
    property int anchor: -1
    property int current: 0
    property string mode: "grid"           // grid | list | columns
    property string sortBy: "name"
    property bool descending: false
    // Hidden files: one setting for every tab and window.
    readonly property bool hidden: Session.hidden
    // Where this folder's view comes from: {own, subfolders, from} (views.rs).
    property var viewInfo: ({ own: false, subfolders: false, from: "" })
    property bool applying: false
    property string filter: ""
    // The Trash itself ("trash:"): restore and delete for good instead of
    // the usual file actions.
    readonly property bool inTrash: path === "trash:"
    // The Storage page ("drives:"): every drive, how full.
    readonly property bool inDrives: path === "drives:"
    // Not a real folder: nothing can be made, pasted or dropped here.
    // The Start page ("start:"): StartMode draws it.
    readonly property bool inStart: path === "start:"
    readonly property bool special: inTrash || inDrives || inStart
    // A tag's files ("tag:Name"): real files, but nothing can be made or
    // pasted here; dropping things here tags them.
    readonly property bool inTag: path.startsWith("tag:")
    // Recent ("recent:"): likewise, files from all over.
    readonly property bool inRecent: path === "recent:"
    // Files gathered from many folders: nothing can be made or pasted here.
    readonly property bool inCollection: inTag || inRecent
    // Git (Settings › Files › General): {root, branch, ahead, behind,
    // ignored, entries: {name: state}}; root "" when not in a repository.
    property var git: ({ root: "", branch: "", ahead: 0, behind: 0, ignored: false, entries: {} })
    readonly property bool wantsGit: Session.git && !pane.special && !pane.inCollection && !pane.searching && pane.path.startsWith("/")
    // An entry's state ("" none; inside an ignored folder: ignored).
    function gitOf(e) {
        if (!pane.wantsGit || pane.git.root === "" || !e) return "";
        if (pane.git.ignored) return "ignored";
        return pane.git.entries[e.name] ?? "";
    }
    function askGit() { if (pane.wantsGit && pane.visible) Ops.askGit(pane.path) }
    onWantsGitChanged: if (!pane.wantsGit) pane.git = { root: "", branch: "", ahead: 0, behind: 0, ignored: false, entries: {} }; else pane.askGit()
    Connections {
        target: Ops
        function onGitStatus(dir, result) { if (dir === pane.path) pane.git = JSON.parse(result) }
    }
    // Files edited elsewhere don't change the folder: look again every few
    // seconds while in a repository and Files is in front.
    Timer {
        interval: 4000
        repeat: true
        running: pane.visible && pane.git.root !== "" && pane.wantsGit && Qt.application.state === Qt.ApplicationActive
        onTriggered: pane.askGit()
    }
    // Folder sizes as they're worked out ({path: {bytes, items}}).
    property var sizes: ({})
    property var sizesSoon: ({})
    readonly property string title: searching ? "Search" : path === Fs.home() ? "Home" : path === "/" ? "/" : inTrash ? "Trash" : inRecent ? "Recent" : inStart ? "Start"
        : JSON.parse(Fs.crumbs(path)).slice(-1)[0]?.name ?? path.substring(path.lastIndexOf("/") + 1)
    // Searching (Search.qml asks, its hits replace the listing): {text,
    // scope, kind, size, modified}; null when not.
    property var search: null
    property var results: []
    property bool searchDone: true
    // Why the search can't run (a regular expression that doesn't parse).
    property string searchError: ""
    property real searchSeen: 0
    property int searchId: -1
    readonly property bool searching: pane.search !== null
    // Group by (kind, modified, size, name; "" none): the items in group
    // order with headings, folded groups shown by their heading only.
    property string group: ""
    property var folded: ({})
    readonly property bool grouped: pane.group !== "" && !pane.searching && !pane.special
        && ["grid", "list", "cards"].includes(pane.mode)
    function fold(label) { const f = Object.assign({}, pane.folded); f[label] = !f[label]; pane.folded = f }
    readonly property var filtered: pane.searching ? pane.results
        : pane.filter === "" ? pane.entries
        : pane.entries.filter(e => e.name.toLowerCase().includes(pane.filter.toLowerCase()))
    readonly property var shownEntries: pane.grouped
        ? Arrange.arrange(pane.filtered, pane.group, false, Date.now())
        : pane.filtered
    readonly property var selection: pane.shownEntries.filter(e => pane.selected[e.path])

    signal openFile(var entry)
    // Used (clicked, keys, navigated): in a split tab, this side becomes the one in use.
    signal activated()
    onActiveFocusChanged: if (pane.activeFocus) pane.activated()
    // Right-click: on `entry` (null: the folder itself), at x, y in the pane.
    signal menuAt(var entry, real x, real y)
    // Files dropped on `dest` (this folder, or a folder in it).
    signal dropped(var paths, string dest)
    // Anything worth keeping changed (Session saves it).
    signal changed()
    // A folder is shown the way it was last time (or as the folder above
    // it lends, or the default); load() follows.
    // (Real folders are remembered for jumping: the address bar's few-letters search.)
    onPathChanged: { if (pane.path.startsWith("/")) Fs.visited(pane.path); pane.git = { root: "", branch: "", ahead: 0, behind: 0, ignored: false, entries: {} }; pane.folded = {}; pane.sizes = {}; pane.sizesSoon = {}; pane.applyView(); pane.changed() }

    // Recent starts as a list, newest first, grouped by day (until changed by hand).
    readonly property var baseView: pane.path === "recent:" ? { mode: "list", sortBy: "modified", descending: true, group: "modified" } : Session.defaultView
    function applyView() {
        const v = JSON.parse(Fs.view(pane.path, JSON.stringify(pane.baseView)));
        pane.applying = true;
        pane.mode = v.mode;
        pane.sortBy = v.sortBy;
        pane.group = v.group ?? "";
        pane.descending = v.descending;
        pane.applying = false;
        pane.viewInfo = { own: v.own, subfolders: v.subfolders, from: v.from };
        pane.picked = false;
    }
    // The view was picked to suit what's in the folder (Layout.adaptive),
    // not set: nothing's remembered until it's changed by hand.
    property bool picked: false
    function adapt() {
        if (!Session.adaptive || pane.viewInfo.own || pane.viewInfo.from !== "" || pane.special || pane.searching || pane.inRecent) return;
        const m = Layout.adaptive(pane.entries, JSON.parse(Fs.view(pane.path, JSON.stringify(Session.defaultView))).mode);
        if (m !== pane.mode) { pane.applying = true; pane.mode = m; pane.applying = false }
        pane.picked = true;
    }
    // Changed here ({mode, sortBy, descending}, any of them): remembered for this folder.
    function setView(changes) {
        pane.applying = true;
        for (const k in changes) pane[k] = changes[k];
        pane.applying = false;
        Fs.setView(pane.path, JSON.stringify({ mode: pane.mode, sortBy: pane.sortBy, descending: pane.descending, group: pane.group }));
        pane.viewInfo = { own: true, subfolders: pane.viewInfo.subfolders, from: "" };
        pane.load();
    }
    // Something about where views come from changed: show it again.
    function refreshView() { pane.applyView(); pane.load() }

    // What a tab is, to save, duplicate, or move to another window.
    function snapshot() {
        return { path: pane.path, back: pane.back.slice(-50), forward: pane.forward.slice(0, 50) };
    }

    // Searches (changes to the question run it again); what's asked is
    // merged with what was.
    function find(ask) {
        pane.activated();
        pane.search = Object.assign({ text: "", scope: "here", kind: "", size: "", modified: "", case: false, regex: false }, pane.search ?? {}, ask);
        pane.searchError = "";
        pane.filter = "";
        pane.selected = {};
        pane.current = 0;
        // Nothing to look for yet: nothing found.
        const s = pane.search;
        if (s.text.trim() === "" && !s.kind && !s.size && !s.modified) {
            Search.stop();
            pane.results = [];
            pane.searchDone = true;
            return;
        }
        pane.searchDone = false;
        pane.searchId = Search.find(JSON.stringify(Object.assign({ here: pane.path, hidden: pane.hidden }, s)));
    }
    function endSearch() {
        if (!pane.searching) return;
        Search.stop();
        pane.search = null;
        pane.results = [];
        pane.searchId = -1;
        pane.load();
    }
    Connections {
        target: Search
        function onFound(id, result) {
            if (id !== pane.searchId) return;
            const r = JSON.parse(result);
            pane.results = r.hits;
            pane.searchSeen = r.seen;
            pane.searchDone = r.done;
            pane.searchError = r.error ?? "";
        }
    }

    function load() {
        // Searching: the same search again (things may have changed).
        if (pane.searching) { pane.find({}); return }
        if (pane.inStart) { pane.error = ""; pane.entries = []; pane.stamp = 0; return }
        if (pane.inDrives) {
            const text = Fs.storage();
            pane.error = "";
            pane.stamp = text.length;
            pane.entries = JSON.parse(text).map(d => Object.assign(d, { dir: true, mime: "inode/directory", size: 0, modified: 0, hidden: false, link: false }));
            return;
        }
        const r = JSON.parse(Fs.list(pane.path, pane.hidden, pane.sortBy, pane.descending, 48, Session.foldersAt));
        if (!r.ok) { pane.error = r.error; pane.entries = []; return }
        pane.error = "";
        pane.entries = r.entries;
        pane.stamp = r.stamp;
        // Keep what's still there selected.
        const still = {};
        for (const e of r.entries) if (pane.selected[e.path]) still[e.path] = true;
        pane.selected = still;
        pane.current = Math.min(pane.current, Math.max(0, r.entries.length - 1));
        pane.adapt();
        pane.askGit();
    }
    function go(p, keepForward) {
        pane.activated();
        if (!p || p === pane.path) { pane.endSearch(); return }
        if (pane.searching) { pane.search = null; pane.results = []; Search.stop() }
        pane.back = pane.back.concat([pane.path]);
        if (!keepForward) pane.forward = [];
        // Coming back up: the folder we were in is selected.
        const from = pane.path;
        pane.path = p;
        pane.filter = "";
        pane.selected = {};
        pane.load();
        const i = pane.entries.findIndex(e => e.path === from);
        pane.current = Math.max(0, i);
        if (i >= 0) { const s = {}; s[from] = true; pane.selected = s; pane.anchor = i }
    }
    function goBack() {
        if (pane.back.length === 0) return;
        const p = pane.back[pane.back.length - 1];
        pane.forward = [pane.path].concat(pane.forward);
        pane.back = pane.back.slice(0, -1);
        const from = pane.path;
        pane.path = p;
        pane.selected = {};
        pane.load();
        const i = pane.entries.findIndex(e => e.path === from);
        pane.current = Math.max(0, i);
    }
    function goForward() {
        if (pane.forward.length === 0) return;
        const p = pane.forward[0];
        pane.forward = pane.forward.slice(1);
        pane.go(p, true);
    }
    function up() {
        const u = Fs.up(pane.path);
        if (u !== "") pane.go(u);
    }
    function activate(entry) {
        if (!entry) return;
        if (entry.dir) pane.go(entry.path);
        else pane.openFile(entry);
    }
    // A click: selects, or (Settings › Files › General: single click) opens
    // what's clicked unless Ctrl or Shift is held.
    function pick(i, ctrl, shift) {
        pane.clickAt(i, ctrl, shift);
        if (Session.openWith === "single" && !ctrl && !shift && pane.shownEntries[i]) {
            const e = pane.shownEntries[i];
            Qt.callLater(() => pane.activate(e));
        }
    }
    function clickAt(i, ctrl, shift) {
        pane.activated();
        const r = F.click(pane.shownEntries, pane.selected, pane.anchor, i, ctrl, shift);
        pane.selected = r.selected;
        pane.anchor = r.anchor;
        pane.current = i;
    }
    // Selects these (after a paste, a rename, a new folder), once listed.
    function selectPaths(paths) {
        pane.load();
        const s = {};
        let first = -1;
        pane.shownEntries.forEach((e, i) => { if (paths.includes(e.path)) { s[e.path] = true; if (first < 0) first = i } });
        pane.selected = s;
        if (first >= 0) { pane.current = first; pane.anchor = first }
    }
    function selectAll() {
        const s = {};
        for (const e of pane.shownEntries) s[e.path] = true;
        pane.selected = s;
    }
    onHiddenChanged: pane.load()
    // A new view takes the keys (if this tab is the one showing).
    onModeChanged: if (pane.visible) Qt.callLater(pane.focusView)
    Component.onCompleted: { pane.applyView(); pane.load() }

    // Something changed in the folder: list it again (checked every 1.5 s
    // while the window is in front; a stat call, nothing more).
    Timer {
        interval: 1500
        repeat: true
        running: pane.visible && Qt.application.state === Qt.ApplicationActive
        onTriggered: {
            if (pane.searching) return;
            // Drives come and go (and fill up): looked at again each time.
            if (pane.inDrives || Fs.stamp(pane.path) !== pane.stamp) pane.load();
        }
    }

    // Dropped on the folder's empty space: into this folder. (Behind the
    // view, so a folder in it takes drops meant for it.)
    FolderDrop {
        id: paneDrop
        anchors.fill: parent
        dest: pane.inDrives ? "" : pane.path
        onFilesDropped: (p, d) => pane.dropped(p, d)
    }
    Rectangle {
        anchors { fill: parent; margins: 4 }
        visible: paneDrop.over
        z: 10
        radius: 12
        color: "transparent"
        border { width: 2; color: Theme.tint(0.5) }
    }

    // ---- the view ---------------------------------------------------------------
    Loader {
        id: view
        anchors.fill: parent
        focus: true
        // Search results come as a list (where each is matters).
        sourceComponent: pane.inStart ? startView : pane.inDrives ? drivesView
            : pane.mode === "list" || pane.searching ? listView
            : pane.grouped && pane.mode === "grid" ? groupedGridView
            : pane.grouped && pane.mode === "cards" ? groupedCardsView
            : ({ columns: columnsView, gallery: galleryView, cards: cardsView, usage: usageView })[pane.mode] ?? gridView
    }
    // (Inside the views `pane` names their own property, hence view.parent.)
    Component { id: gridView; GridMode { pane: view.parent } }
    Component { id: listView; ListMode { pane: view.parent } }
    Component { id: columnsView; ColumnsMode { pane: view.parent } }
    Component { id: drivesView; DrivesMode { pane: view.parent } }
    Component { id: startView; StartMode { pane: view.parent } }
    Component { id: groupedGridView; GroupedGrid { pane: view.parent } }
    Component { id: groupedCardsView; GroupedGrid { pane: view.parent; cards: true } }
    Component { id: galleryView; GalleryMode { pane: view.parent } }
    Component { id: cardsView; CardsMode { pane: view.parent } }
    Component { id: usageView; UsageMode { pane: view.parent } }

    // Folder sizes for the list's Size column, worked out in the background
    // (not for / : that would be the whole system).
    readonly property bool wantsSizes: (["list", "cards"].includes(pane.mode) && Session.folderSizes || pane.mode === "usage") && !pane.special && !pane.searching && pane.path !== "/"
    onEntriesChanged: pane.askSizes()
    onWantsSizesChanged: pane.askSizes()
    function askSizes() {
        if (!pane.wantsSizes || !pane.visible) return;
        Ops.sizeFolders(JSON.stringify(pane.entries.filter(e => e.dir && !e.link).map(e => e.path)));
    }
    onVisibleChanged: if (pane.visible) pane.askSizes()
    Connections {
        target: Ops
        function onFolderSized(path, bytes, items) {
            if (!pane.wantsSizes || !path.startsWith(pane.path + "/")) return;
            pane.sizesSoon[path] = { bytes: bytes, items: items };
            sizesTimer.restart();
        }
    }
    // Shown a few at a time (not a redraw per folder).
    Timer { id: sizesTimer; interval: 120; onTriggered: pane.sizes = Object.assign({}, pane.sizesSoon) }
    function focusView() { view.forceActiveFocus(); if (view.item) view.item.forceActiveFocus() }

    // Typing filters the folder by name (Backspace takes letters off, Esc
    // clears); Space starts nothing until something's typed.
    Keys.onPressed: e => {
        if (pane.searching) return;
        const mods = e.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier);
        if (e.key === Qt.Key_Escape && pane.filter !== "") { pane.filter = ""; e.accepted = true; return }
        if (e.key === Qt.Key_Backspace && pane.filter !== "") { pane.filter = pane.filter.slice(0, -1); e.accepted = true; return }
        if (mods || !e.text || e.text.length !== 1 || e.text < " " || e.text === "\x7f") return;
        if (e.text === " " && pane.filter === "") return;
        pane.filter += e.text;
        pane.current = 0;
        e.accepted = true;
    }
    // What's being filtered by, and taking it further.
    Rectangle {
        visible: pane.filter !== "" && !pane.searching
        z: 15
        anchors { left: parent.left; bottom: parent.bottom; margins: 12 }
        height: 36; radius: 12
        width: filterRow.implicitWidth + 16
        color: Theme.surface1
        border { width: 1; color: Theme.surface2 }
        Row {
            id: filterRow
            anchors.centerIn: parent
            spacing: 6
            Icon { anchors.verticalCenter: parent.verticalCenter; name: "search"; size: 13; color: Theme.muted }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: pane.filter
                color: Theme.text
                font { family: Theme.font; pointSize: Theme.pt(12.5); weight: Font.Medium }
            }
            Item { width: 4; height: 1 }
            Button { text: "In subfolders"; kind: "ghost"; onClicked: pane.find({ text: pane.filter, scope: "here" }) }
            Button { text: "Everywhere"; kind: "ghost"; onClicked: pane.find({ text: pane.filter, scope: "everywhere" }) }
            IconButton { width: 26; height: 26; icon: "x"; iconSize: 11; fill: "transparent"; anchors.verticalCenter: parent.verticalCenter; onClicked: { pane.filter = ""; pane.focusView() } }
        }
    }

    // Empty, or can't be opened.
    Column {
        anchors.centerIn: parent
        visible: !pane.inStart && (pane.error !== "" || (pane.shownEntries.length === 0 && (pane.mode !== "columns" || pane.searching)))
        spacing: 8
        Icon {
            anchors.horizontalCenter: parent.horizontalCenter
            name: pane.error ? "lock" : pane.searching ? "search" : "folder"
            size: 34; color: Theme.faint
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: pane.error || pane.searchError || (pane.searching ? (!pane.searchDone ? "Looking…" : (pane.search.text.trim() || pane.search.kind || pane.search.size || pane.search.modified) ? "Nothing found" : "")
                : pane.filter !== "" ? "Nothing matches “" + pane.filter + "”" : "This folder is empty")
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(13) }
        }
    }
}
