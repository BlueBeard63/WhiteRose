// One tab: a folder, its history (back / forward / up), its entries in the
// chosen view (grid, list or columns), and the selection. The listing
// refreshes by itself when files are added, removed or renamed.
import QtQuick
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F

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
    readonly property string title: searching ? "Search" : path === Fs.home() ? "Home" : path === "/" ? "/" : inTrash ? "Trash"
        : JSON.parse(Fs.crumbs(path)).slice(-1)[0]?.name ?? path.substring(path.lastIndexOf("/") + 1)
    // Searching (Search.qml asks, its hits replace the listing): {text,
    // scope, kind, size, modified}; null when not.
    property var search: null
    property var results: []
    property bool searchDone: true
    property real searchSeen: 0
    property int searchId: -1
    readonly property bool searching: pane.search !== null
    readonly property var shownEntries: pane.searching ? pane.results
        : pane.filter === "" ? pane.entries
        : pane.entries.filter(e => e.name.toLowerCase().includes(pane.filter.toLowerCase()))
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
    onPathChanged: { pane.applyView(); pane.changed() }

    function applyView() {
        const v = JSON.parse(Fs.view(pane.path));
        pane.applying = true;
        pane.mode = v.mode;
        pane.sortBy = v.sortBy;
        pane.descending = v.descending;
        pane.applying = false;
        pane.viewInfo = { own: v.own, subfolders: v.subfolders, from: v.from };
    }
    // Changed here ({mode, sortBy, descending}, any of them): remembered for this folder.
    function setView(changes) {
        pane.applying = true;
        for (const k in changes) pane[k] = changes[k];
        pane.applying = false;
        Fs.setView(pane.path, JSON.stringify({ mode: pane.mode, sortBy: pane.sortBy, descending: pane.descending }));
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
        pane.search = Object.assign({ text: "", scope: "here", kind: "", size: "", modified: "" }, pane.search ?? {}, ask);
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
        }
    }

    function load() {
        // Searching: the same search again (things may have changed).
        if (pane.searching) { pane.find({}); return }
        const r = JSON.parse(Fs.list(pane.path, pane.hidden, pane.sortBy, pane.descending, 48));
        if (!r.ok) { pane.error = r.error; pane.entries = []; return }
        pane.error = "";
        pane.entries = r.entries;
        pane.stamp = r.stamp;
        // Keep what's still there selected.
        const still = {};
        for (const e of r.entries) if (pane.selected[e.path]) still[e.path] = true;
        pane.selected = still;
        pane.current = Math.min(pane.current, Math.max(0, r.entries.length - 1));
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
        onTriggered: if (!pane.searching && Fs.stamp(pane.path) !== pane.stamp) pane.load()
    }

    // Dropped on the folder's empty space: into this folder. (Behind the
    // view, so a folder in it takes drops meant for it.)
    FolderDrop {
        id: paneDrop
        anchors.fill: parent
        dest: pane.path
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
        sourceComponent: pane.mode === "list" || pane.searching ? listView : pane.mode === "columns" ? columnsView : gridView
    }
    // (Inside the views `pane` names their own property, hence view.parent.)
    Component { id: gridView; GridMode { pane: view.parent } }
    Component { id: listView; ListMode { pane: view.parent } }
    Component { id: columnsView; ColumnsMode { pane: view.parent } }
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
        visible: pane.error !== "" || (pane.shownEntries.length === 0 && (pane.mode !== "columns" || pane.searching))
        spacing: 8
        Icon {
            anchors.horizontalCenter: parent.horizontalCenter
            name: pane.error ? "lock" : pane.searching ? "search" : "folder"
            size: 34; color: Theme.faint
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: pane.error || (pane.searching ? (!pane.searchDone ? "Looking…" : (pane.search.text.trim() || pane.search.kind || pane.search.size || pane.search.modified) ? "Nothing found" : "")
                : pane.filter !== "" ? "Nothing matches “" + pane.filter + "”" : "This folder is empty")
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(13) }
        }
    }
}
