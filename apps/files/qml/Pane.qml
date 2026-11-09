// One tab: a folder, its history (back / forward / up), its entries in the
// chosen view (grid, list or columns), and the selection. The listing
// refreshes by itself when files are added, removed or renamed.
import QtQuick
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F

Item {
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
    property bool hidden: false
    property string filter: ""
    readonly property string title: path === Fs.home() ? "Home" : path === "/" ? "/" : path.substring(path.lastIndexOf("/") + 1)
    readonly property var shownEntries: pane.filter === "" ? pane.entries
        : pane.entries.filter(e => e.name.toLowerCase().includes(pane.filter.toLowerCase()))
    readonly property var selection: pane.shownEntries.filter(e => pane.selected[e.path])

    signal openFile(var entry)
    // Right-click: on `entry` (null: the folder itself), at x, y in the pane.
    signal menuAt(var entry, real x, real y)
    // Anything worth keeping changed (Session saves it).
    signal changed()
    onPathChanged: pane.changed()
    onModeChanged: pane.changed()

    // What a tab is, to save, duplicate, or move to another window.
    function snapshot() {
        return { path: pane.path, back: pane.back.slice(-50), forward: pane.forward.slice(0, 50),
                 mode: pane.mode, sortBy: pane.sortBy, descending: pane.descending, hidden: pane.hidden };
    }

    function load() {
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
        if (!p || p === pane.path) return;
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
        if (pane.path === "/") return;
        const i = pane.path.lastIndexOf("/");
        pane.go(i <= 0 ? "/" : pane.path.substring(0, i));
    }
    function activate(entry) {
        if (!entry) return;
        if (entry.dir) pane.go(entry.path);
        else pane.openFile(entry);
    }
    function clickAt(i, ctrl, shift) {
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
    onHiddenChanged: { load(); pane.changed() }
    onSortByChanged: { load(); pane.changed() }
    onDescendingChanged: { load(); pane.changed() }
    Component.onCompleted: load()

    // Something changed in the folder: list it again (checked every 1.5 s
    // while the window is in front; a stat call, nothing more).
    Timer {
        interval: 1500
        repeat: true
        running: pane.visible && Qt.application.state === Qt.ApplicationActive
        onTriggered: if (Fs.stamp(pane.path) !== pane.stamp) pane.load()
    }

    // ---- the view ---------------------------------------------------------------
    Loader {
        id: view
        anchors.fill: parent
        focus: true
        sourceComponent: pane.mode === "list" ? listView : pane.mode === "columns" ? columnsView : gridView
    }
    // (Inside the views `pane` names their own property, hence view.parent.)
    Component { id: gridView; GridMode { pane: view.parent } }
    Component { id: listView; ListMode { pane: view.parent } }
    Component { id: columnsView; ColumnsMode { pane: view.parent } }
    function focusView() { view.forceActiveFocus(); if (view.item) view.item.forceActiveFocus() }

    // Empty, or can't be opened.
    Column {
        anchors.centerIn: parent
        visible: pane.error !== "" || (pane.shownEntries.length === 0 && pane.mode !== "columns")
        spacing: 8
        Icon { anchors.horizontalCenter: parent.horizontalCenter; name: pane.error ? "lock" : "folder"; size: 34; color: Theme.faint }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: pane.error || (pane.filter !== "" ? "Nothing matches “" + pane.filter + "”" : "This folder is empty")
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(13) }
        }
    }
}
