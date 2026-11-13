// Shell-wide UI state, also reachable from scripts and keybindings:
//   qs -p shell ipc call island set dock|clipboard|screenshots|launcher|idle
//   qs -p shell ipc call dock toggle|open|close|preview <app-id>
//   qs -p shell ipc call launcher toggle|open|close|search <text>
//   qs -p shell ipc call quick toggle|open|close
//   qs -p shell ipc call notifications toggle|open|close
//   qs -p shell ipc call calendar toggle|open|close
//   qs -p shell ipc call session lock
//   qs -p shell ipc call zones edit <monitor id> <workspace 1-9 or ""> <tool>
//   qs -p shell ipc call zones done
pragma Singleton
import QtQuick
import Quickshell
import "Report.js" as Rep
import Quickshell.Io

Singleton {
    id: root

    // Which view the centre island shows. The window switcher overrides this
    // while ALT+TAB is held (it is driven by the compositor).
    property string islandView: "idle"
    // The island's views: one file each in shell/island/ (the name is the
    // file's; see IslandView.qml), found when the shell starts.
    property var islandViews: []
    readonly property var views: ["idle"].concat(islandViews)
    // A prompt that came before the views were found (a crash, memory
    // running out, right as the shell starts): offered once they are.
    onIslandViewsChanged: { offerCrash(); offerMemory(); offerKeyring() }
    Process {
        running: true
        command: ["sh", "-c", "for f in \"$1\"/*.qml; do [ -f \"$f\" ] && basename \"$f\" .qml; done; true", "sh", Quickshell.shellDir + "/island"]
        stdout: StdioCollector { onStreamFinished: root.islandViews = text.split("\n").filter(n => /^[a-z][a-z0-9-]*$/.test(n)) }
    }

    // The screen (connector) a view is for, if only one: the others' islands
    // stay at rest. "" for every screen.
    property string islandScreen: ""
    // (islandViews, not views: read while islandViews is changing, views
    // can still be the old list.)
    function show(view) { if (view === "idle" || islandViews.includes(view)) { islandView = view; islandScreen = ""; if (view !== "idle") panel = "" } }
    function toggle(view) { islandView === view ? close() : show(view) }
    function close() { islandView = "idle"; islandScreen = "" }

    // A monitor's snap zones edited on screen (Settings › Snapping, "Edit on
    // screen"): { monitor: its Compositor.outputs id, ws: "" for all
    // workspaces' grid or "1"-"9" for one's own, tool }, or null. The zones
    // are drawn over that monitor (ZonesOverlay.qml) and the tools are in
    // its island (island/zones.qml). Ending it any way ends both.
    property var zones: null
    // The tools' size in the island (ZoneTools.qml), so the zones overlay
    // leaves the island hanging over it clickable.
    property size zoneTools: Qt.size(0, 0)
    function editZones(monitor, ws, tool) {
        const out = Compositor.outputs.find(o => o.id === monitor);
        if (!out) return;
        show("zones");
        zones = { monitor, ws: /^[1-9]$/.test(ws) ? ws : "", tool: tool || "resize" };
        // Its island, if that monitor has a bar (else every island shows it).
        const bars = Daemon.setting("bar.screens", "all") !== "main" || Compositor.outputs.find(o => o.enabled)?.connector === out.connector;
        islandScreen = bars ? out.connector : "";
    }
    function zonesTool(tool) { if (zones) zones = Object.assign({}, zones, { tool }) }
    function endZones() { if (islandView === "zones") close(); zones = null }
    // Another view (or none) taking the island ends the editing.
    onIslandViewChanged: {
        if (islandView !== "zones") zones = null;
        // A crash's prompt closed without an answer: offered again later.
        if (islandView !== "reportprompt" && reportPrompt && !promptAnswered && reportPrompt.what !== "manual") {
            crashWaiting = reportPrompt;
            reportPrompt = null;
        }
        // A crash or a memory warning waiting: now, if the island is free.
        offerCrash();
        if (islandView === "idle") { offerMemory(); offerKeyring(); }
    }

    // The side panel hanging from the bar: "" (none) or "quick". Only one
    // thing is open at a time: opening it returns the island to idle.
    property string panel: ""
    function togglePanel(name) { if (panel === name) closePanel(); else { panel = name; islandView = "idle" } }
    function closePanel() { panel = "" }

    // Opens the Settings app (its own window), at `page` if given.
    function openSettings(page) {
        panel = ""; islandView = "idle";
        Compositor.launch("whiterose-settings", [Quickshell.shellDir + "/bin/whiterose-settings", page ?? ""]);
    }

    // The session lock screen (LockScreen.qml) shows while this is true; only
    // a correct password clears it.
    property bool locked: false
    function lock() { panel = ""; islandView = "idle"; locked = true }

    // A new WhiteRose version didn't start (the session launcher went back):
    // { version, reason, attempts, tries, log, back_on }, from whiterosed.
    // The island shows the "couldn't start" prompt once per session; Try
    // again / Stay settle it (Settings › Updates shows it too).
    property var updateFailure: null
    property bool failureShown: false
    function noteUpdates(st) {
        root.updateFailure = st?.failure ?? null;
        if (root.updateFailure && !root.failureShown && !root.locked) {
            root.failureShown = true;
            root.show("recovery");
        } else if (!root.updateFailure && root.islandView === "recovery") {
            root.close();
        }
    }
    Connections {
        target: Daemon
        function onUpdatesEvent(st) { root.noteUpdates(st) }
        function onConnectedChanged() { root.askUpdates() }
    }
    function askUpdates() {
        if (Daemon.connected) Daemon.request({ op: "updates" }, v => { if (v) root.noteUpdates(v) });
    }
    // (Already connected when this was made: asked now.)
    Component.onCompleted: { askUpdates(); askMemory(); askCrashes() }

    // An item is being dragged out of the island: keep it open meanwhile.
    property bool dragging: false

    // Desktop entry id whose windows the dock should preview (hover, or IPC).
    property string dockPreviewKey: ""
    // Text to pre-fill when the launcher opens (from `launcher search <text>`).
    property string launcherQuery: ""

    IpcHandler {
        target: "island"
        function set(view: string): void { root.show(view) }
    }
    IpcHandler {
        target: "dock"
        function toggle(): void { root.toggle("dock") }
        function open(): void { root.show("dock") }
        function close(): void { root.close() }
        function preview(id: string): void { root.show("dock"); root.dockPreviewKey = id }
    }
    IpcHandler {
        target: "quick"
        function toggle(): void { root.togglePanel("quick") }
        function open(): void { if (root.panel !== "quick") root.togglePanel("quick") }
        function close(): void { root.closePanel() }
    }
    IpcHandler {
        target: "notifications"
        function toggle(): void { root.togglePanel("notifications") }
        function open(): void { if (root.panel !== "notifications") root.togglePanel("notifications") }
        function close(): void { root.closePanel() }
    }
    IpcHandler {
        target: "calendar"
        function toggle(): void { root.togglePanel("calendar") }
        function open(): void { if (root.panel !== "calendar") root.togglePanel("calendar") }
        function close(): void { root.closePanel() }
    }
    IpcHandler {
        target: "zones"
        function edit(monitor: string, ws: string, tool: string): void { root.editZones(monitor, ws, tool) }
        function done(): void { root.endZones() }
    }
    // The bug report flow (island/report.qml): `from` is the crash record
    // (snapshot) it starts on, "" to pick.
    property string reportFrom: ""
    function report(from) { reportFrom = from ?? ""; show("report") }
    // "Report this" after a crash / "Snapshot saved" (island/reportprompt.qml):
    // the record ({ id, what, title, time }). A crash waits for the island
    // to be free (not over the launcher you're typing in, not while locked)
    // and is offered until it's answered (Report or Not now: whiterosed
    // remembers, across shell restarts). Closed some other way (another
    // view, Settings opening), it's offered again once the island is free.
    property var reportPrompt: null
    property var crashWaiting: null
    property bool promptAnswered: false
    function promptReport(rec) {
        reportPrompt = rec;
        promptAnswered = false;
        show("reportprompt");
    }
    // Report (`report` true) or Not now / Later.
    function answerPrompt(report) {
        const rec = reportPrompt;
        promptAnswered = true;
        if (rec && rec.what !== "manual") Daemon.request({ op: "report_prompted", name: rec.id }, () => {});
        if (report && rec) root.report(rec.id); else close();
    }
    function noteCrashes(list) {
        const c = Rep.crashToPrompt(list, Date.now());
        if (c) crashWaiting = c;
        offerCrash();
    }
    function offerCrash() {
        if (crashWaiting && islandView === "idle" && !locked) {
            const c = crashWaiting;
            crashWaiting = null;
            promptReport(c);
        }
    }
    onLockedChanged: { offerCrash(); offerMemory(); offerKeyring() }
    // SUPER+SHIFT+B: a snapshot of this moment, then the offer to report it.
    function snapshotNow() {
        Daemon.request({ op: "report_snapshot_now" }, (v, err) => {
            if (err || !v) return;
            root.promptReport({ id: v.id, what: "manual", title: "Snapshot you took", time: Date.now() });
        });
    }
    // The keyring is locked after a fingerprint login (island/keyring.qml):
    // asked once a login, over nothing but the idle island. Not now (or
    // clicking away) asks no more; it goes away if the keyring opens anyway
    // (the lock screen's password opens it too).
    property bool keyringAsked: false
    function offerKeyring() {
        if (System.keyringLocked && !keyringAsked && islandView === "idle" && !locked && islandViews.includes("keyring")) {
            keyringAsked = true;
            show("keyring");
        }
    }
    Connections {
        target: System
        function onKeyringLockedChanged() {
            if (System.keyringLocked) root.offerKeyring();
            else if (root.islandView === "keyring") root.close();
        }
    }
    // Memory running out (island/memory.qml): { level, app, available, total }.
    // Shown over nothing but the idle island or itself; else it waits.
    property var memoryWarning: null
    function offerMemory() {
        if (memoryWarning && (islandView === "idle" || islandView === "memory") && !locked) show("memory");
    }
    Connections {
        target: Daemon
        function onMemoryWarning(e) { root.memoryWarning = e; root.offerMemory() }
        // Connected (or reconnected after a restart): the warning showing, if any.
        function onConnectedChanged() { root.askMemory() }
    }
    function askCrashes() {
        if (Daemon.connected) Daemon.request({ op: "report_snapshots" }, v => { if (v) root.noteCrashes(v) });
    }
    function askMemory() {
        if (Daemon.connected) Daemon.request({ op: "memory_warning" }, v => { if (v) { root.memoryWarning = v; root.offerMemory(); } });
    }

    Connections {
        target: Daemon
        function onReportRequested(from) { root.report(from) }
        function onReportSnapshotsEvent(list) { root.noteCrashes(list) }
        function onConnectedChanged() { root.askCrashes() }
    }
    IpcHandler {
        target: "report"
        function open(from: string): void { root.report(from) }
    }
    IpcHandler {
        target: "session"
        function lock(): void { root.lock() }
    }
    IpcHandler {
        target: "launcher"
        function toggle(): void { root.toggle("launcher") }
        function open(): void { root.show("launcher") }
        function close(): void { root.close() }
        function search(text: string): void {
            root.launcherQuery = text;
            if (root.islandView === "launcher") root.launcherQueryChanged(); else root.show("launcher");
        }
    }
}
