// Every open Files window in this process, and what's kept between runs:
// the tabs of all windows (saved a moment after anything changes, and as
// the last window closes) and the tabs closed recently, to reopen.
//
// Only a Files started without folders owns the saved session (restores it,
// keeps it up to date); one opened on a folder from elsewhere leaves it be.
pragma Singleton
import QtQuick
import WhiteRose.Kit
import WhiteRose.Files

QtObject {
    id: session
    property bool owner: false
    // Hidden files shown, everywhere.
    // ---- settings (whiterosed, Settings › Files): followed as they change -----
    // A setting's value, or `fallback` if it isn't there (no daemon).
    function setting(key, fallback) {
        try { const v = JSON.parse(Desktop.setting(key)); return v === null || v === undefined ? fallback : v } catch (e) { return fallback }
    }
    function set(key, value) { Desktop.set(key, JSON.stringify(value)) }
    property bool hidden: session.setting("files.hidden", false)
    property bool details: session.setting("files.details", false)
    property bool adaptive: session.setting("files.adaptive", true)
    property real zoom: session.setting("files.zoom", 1)
    property string foldersAt: session.setting("files.foldersAt", "first")
    property var columns: session.setting("files.columns", ["size", "modified", "kind"])
    property var widths: session.setting("files.widths", ({}))
    property bool folderSizes: session.setting("files.folderSizes", true)
    property string openWith: session.setting("files.openWith", "double")
    property bool confirmTrash: session.setting("files.confirmTrash", false)
    // Copies read back and checked against the original (Settings › Files › General).
    property bool verifyCopies: session.setting("files.verifyCopies", false)
    // Recent files kept (Settings › Privacy): Recent shows in the sidebar.
    property bool recentFiles: session.setting("privacy.recentFiles", true)
    onVerifyCopiesChanged: Ops.setVerify(session.verifyCopies)
    // Git badges and the branch (Settings › Files › General).
    property bool git: session.setting("files.git", true)
    // The sidebar: shown or hidden (Ctrl+B), and its sections folded away.
    property bool sidebar: session.setting("files.sidebar", true)
    property var sidebarFolded: session.setting("files.sidebarFolded", [])
    // Folders opened in the sidebar's tree.
    property var sidebarOpen: session.setting("files.sidebarOpen", [])
    function toggleSidebarOpen(path) {
        const next = session.sidebarOpen.includes(path) ? session.sidebarOpen.filter(x => x !== path && !x.startsWith(path + "/")) : session.sidebarOpen.concat([path]).slice(-50);
        session.sidebarOpen = next;
        session.set("files.sidebarOpen", next);
    }
    function setSidebar(on) { session.sidebar = on; session.set("files.sidebar", on) }
    function foldSidebar(section) {
        const next = session.sidebarFolded.includes(section) ? session.sidebarFolded.filter(x => x !== section) : session.sidebarFolded.concat([section]);
        session.sidebarFolded = next;
        session.set("files.sidebarFolded", next);
    }
    // Tags: names and colours, in order ({name, colour}).
    property var tagDefs: session.setting("files.tags", [])
    readonly property var palette: ({
        rose: "#E8A3AE", red: "#E5534B", orange: "#F0883E", yellow: "#E3B341", green: "#57AB5A",
        teal: "#39C5BB", blue: "#539BF5", purple: "#986EE2", pink: "#E275AD", grey: "#8B949E",
    })
    // A tag's colour (one Files doesn't list yet: grey).
    function tagColour(name) {
        const d = session.tagDefs.find(t => t.name.toLowerCase() === String(name).toLowerCase());
        return session.palette[d?.colour ?? "grey"];
    }
    function defineTag(name, colour) {
        if (session.tagDefs.some(t => t.name.toLowerCase() === name.toLowerCase())) return;
        const next = session.tagDefs.concat([{ name: name, colour: colour }]);
        session.tagDefs = next;
        session.set("files.tags", next);
    }
    function setTagDefs(list) { session.tagDefs = list; session.set("files.tags", list) }
    // The next colour for a new tag (one not used yet, if any).
    function freshColour() {
        const used = session.tagDefs.map(t => t.colour);
        return Object.keys(session.palette).find(c => !used.includes(c)) ?? "rose";
    }
    property var defaultView: session.setting("files.defaultView", ({ mode: "grid", sortBy: "name", descending: false, group: "" }))
    readonly property var indexOpts: ({ drives: session.setting("files.indexDrives", true), exclude: session.setting("files.indexExclude", []) })
    // The daemon's settings arrive a moment after start (the values above are
    // the defaults until then): everything read again once they're here.
    function readAll() {
        const v = (k, f) => session.setting(k, f);
        session.hidden = v("files.hidden", false);
        session.details = v("files.details", false);
        session.adaptive = v("files.adaptive", true);
        session.zoom = v("files.zoom", 1);
        session.foldersAt = v("files.foldersAt", "first");
        session.columns = v("files.columns", ["size", "modified", "kind"]);
        session.widths = v("files.widths", {});
        session.folderSizes = v("files.folderSizes", true);
        session.openWith = v("files.openWith", "double");
        session.confirmTrash = v("files.confirmTrash", false);
        session.verifyCopies = v("files.verifyCopies", false);
        session.recentFiles = v("privacy.recentFiles", true);
        session.tagDefs = v("files.tags", []);
        session.sidebar = v("files.sidebar", true);
        session.git = v("files.git", true);
        session.sidebarFolded = v("files.sidebarFolded", []);
        session.sidebarOpen = v("files.sidebarOpen", []);
        session.defaultView = v("files.defaultView", session.defaultView);
        session.reload(true);
    }
    property Connections connected: Connections {
        target: Desktop
        function onConnectedChanged() { if (Desktop.connected) session.readAll() }
    }
    // Changed (here, in Settings, or in another Files): every window shows it.
    property Connections watch: Connections {
        target: Desktop
        function onSettingChanged(key) {
            if (!key.startsWith("files.")) return;
            const v = (k, f) => session.setting(k, f);
            switch (key) {
            case "files.hidden": session.hidden = v(key, false); break;
            case "files.details": session.details = v(key, false); break;
            case "files.adaptive": session.adaptive = v(key, true); session.reload(true); break;
            case "files.zoom": session.zoom = v(key, 1); break;
            case "files.foldersAt": session.foldersAt = v(key, "first"); session.reload(false); break;
            case "files.columns": session.columns = v(key, ["size", "modified", "kind"]); break;
            case "files.widths": session.widths = v(key, {}); break;
            case "files.folderSizes": session.folderSizes = v(key, true); break;
            case "files.openWith": session.openWith = v(key, "double"); break;
            case "files.confirmTrash": session.confirmTrash = v(key, false); break;
            case "files.verifyCopies": session.verifyCopies = v(key, false); break;
            case "privacy.recentFiles": session.recentFiles = v(key, true); break;
            case "files.git": session.git = v(key, true); break;
            case "files.sidebar": session.sidebar = v(key, true); break;
            case "files.sidebarFolded": session.sidebarFolded = v(key, []); break;
            case "files.sidebarOpen": session.sidebarOpen = v(key, []); break;
            case "files.tags": session.tagDefs = v(key, []); session.reload(false); break;
            case "files.defaultView": session.defaultView = v(key, session.defaultView); session.reload(true); break;
            case "files.reindex": Search.keepFresh(JSON.stringify(session.indexOpts), v(key, 0)); break;
            }
        }
    }
    // Every folder shown again (`views`: their views looked up again too).
    function reload(views) {
        for (const w of session.windows) for (const t of w.tabs) for (const p of t.panes) { if (views) p.applyView(); p.load() }
    }
    function setHidden(on) { session.hidden = on; session.set("files.hidden", on) }
    function setDetails(on) { session.details = on; session.set("files.details", on) }
    function setAdaptive(on) { session.adaptive = on; session.set("files.adaptive", on) }
    function setZoom(z) { session.zoom = Math.max(0.75, Math.min(1.6, Math.round(z * 100) / 100)); session.set("files.zoom", session.zoom) }
    function setFoldersAt(at) { session.foldersAt = at; session.set("files.foldersAt", at); session.reload(false) }
    function toggleColumn(key) {
        const on = session.columns.includes(key);
        const next = ["size", "modified", "created", "kind", "tags"].filter(k => k === key ? !on : session.columns.includes(k));
        session.columns = next;
        session.set("files.columns", next);
    }
    // While dragging: shown only; `keep`: saved.
    function setWidth(key, w, keep) {
        const next = Object.assign({}, session.widths);
        next[key] = Math.round(Math.max(60, Math.min(600, w)));
        session.widths = next;
        if (keep) session.set("files.widths", next);
    }
    function setDefaultView(v) { session.defaultView = v; session.set("files.defaultView", v); session.reload(true) }
    // Settings › Files (at a sub-page: general, views, search, apps, shortcuts).
    function openSettings(sub) { const e = Fs.openSettings("files" + (sub ? "/" + sub : "")); if (e) console.warn(e) }

    // ---- workspaces: what's open, saved under a name (workspaces.rs) ------------------
    signal workspacesChanged()
    // Saves every open window as `name`: "" or why not.
    function saveWorkspace(name) {
        const err = Fs.saveWorkspace(name, JSON.stringify({ windows: session.windows.map(w => w.snapshot()) }));
        if (err === "") session.workspacesChanged();
        return err;
    }

    property var windows: []
    // Closed tabs, newest last: { tab: snapshot, index }.
    property var closed: []

    // Starts the first window; returns its tabs ({tabs, current}).
    function start() {
        // The search index, made or freshened in the background.
        Search.keepFresh(JSON.stringify(session.indexOpts), session.setting("files.reindex", 0));
        const args = JSON.parse(Fs.arguments());
        if (args.length > 0) return { tabs: args.map(a => ({ path: a })), current: 0 };
        session.owner = true;
        // Where it starts: your last tabs, home, or a chosen folder (the tabs
        // are still saved as you go either way).
        const startIn = session.setting("files.startIn", "last");
        if (startIn === "home") return { tabs: [{ path: "" }], current: 0 };
        if (startIn === "start") return { tabs: [{ path: "start:" }], current: 0 };
        if (startIn === "folder") {
            const f = Fs.resolve(session.setting("files.startFolder", ""));
            return { tabs: [{ path: f }], current: 0 };
        }
        const saved = JSON.parse(Fs.loadSession()).windows;
        if (saved.length === 0) return { tabs: [{ path: "" }], current: 0 };
        // The others get windows of their own.
        for (const w of saved.slice(1)) session.open(w);
        return saved[0];
    }
    // A new window with these tabs ({tabs, current}).
    function open(w) {
        const c = Qt.createComponent("WhiteRose.Files", "Main");
        if (c.status !== Component.Ready) { console.warn(c.errorString()); return null }
        return c.createObject(null, { initial: w });
    }
    // This process, and a number per window: a tab dragged between windows
    // says where it came from.
    readonly property string token: Math.random().toString(36).slice(2) + Date.now().toString(36)
    property int lastUid: 0
    function add(win) {
        session.lastUid += 1;
        win.uid = session.lastUid;
        session.windows = session.windows.concat([win]);
    }
    // A window is closing: the last one is saved as it goes; another one's
    // tabs go with it (the session is the windows still open).
    function remove(win) {
        if (session.windows.length === 1) session.write();
        session.windows = session.windows.filter(w => w !== win);
        if (session.windows.length > 0) session.write();
    }
    function forget(tab, index) {
        session.closed = session.closed.concat([{ tab: tab, index: index }]).slice(-25);
    }
    function reopen() {
        const last = session.closed[session.closed.length - 1];
        session.closed = session.closed.slice(0, -1);
        return last ?? null;
    }

    function changed() { if (session.owner) saveSoon.restart() }
    function write() {
        if (!session.owner) return;
        saveSoon.stop();
        Fs.saveSession(JSON.stringify({ windows: session.windows.map(w => w.snapshot()) }));
    }
    property Timer saveSoon: Timer { interval: 800; onTriggered: session.write() }
}
