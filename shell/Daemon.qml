// Connection to whiterosed, the session daemon ($WHITEROSE_DAEMON_SOCKET):
// the user's settings (one database, every change pushed here as it
// happens), the system state (brightness, radios, weather, calendars) and
// logind's lock requests. Newline-delimited JSON; see
// daemon/internal/server/server.go for the protocol.
pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

Singleton {
    id: root

    readonly property string socketPath: Quickshell.env("WHITEROSE_DAEMON_SOCKET")
        || (Quickshell.env("XDG_RUNTIME_DIR") + "/whiterosed.sock")
    readonly property bool connected: socket.connected

    // Every setting by key ("dock.pinned" -> [...]); defaults included.
    property var settings: ({})
    // The latest system state ({ brightness, airplane, radios, weather, calendars }).
    property var state: null

    // App setup progress: { step, apps, state: running|done|failed, line, error }.
    signal appsEvent(var e)
    // Adding a fingerprint: { finger, state: stage|retry|done|failed|cancelled, stage, stages, message }.
    signal fingerprintEvent(var e)
    // WhiteRose updates: the Updates page's whole status, whenever it changes.
    signal updatesEvent(var status)
    // Settings › About › Report a problem: the island's report flow, in this
    // session's shell (`from`: a crash record to start on, or "").
    signal reportRequested(string from)
    // A crash record got its snapshot (the "Report this" prompt).
    signal reportSnapshotsEvent(var list)
    // Memory is running out (the memory watcher names the app at fault).
    signal memoryWarning(var e)
    signal lockRequested()
    signal unlockRequested()

    // A setting, or `fallback` before the daemon has answered.
    function setting(key, fallback) {
        const v = root.settings[key];
        return v === undefined ? fallback : v;
    }

    // Changes a setting. Shown at once; the daemon checks it, stores it and
    // tells everyone (a rejected value is put back).
    function set(key, value) {
        const next = Object.assign({}, root.settings);
        next[key] = value;
        root.settings = next;
        request({ op: "set", key: key, value: value }, (v, err) => {
            if (err) {
                console.warn("setting", key, "refused:", err);
                request({ op: "get", key: key }, real => root.apply(key, real));
            }
        });
    }

    // Sends a request; `done(value, error)` gets the reply.
    property int nextId: 1
    property var pending: ({})
    function request(req, done) {
        const id = root.nextId++;
        if (done) root.pending[id] = done;
        req.id = id;
        if (!socket.connected) {
            if (done) { delete root.pending[id]; done(undefined, "whiterosed isn't running"); }
            return;
        }
        socket.write(JSON.stringify(req) + "\n");
        socket.flush();
    }

    function apply(key, value) {
        const next = Object.assign({}, root.settings);
        next[key] = value;
        root.settings = next;
    }

    function handle(line) {
        let msg;
        try { msg = JSON.parse(line); } catch (e) { console.warn("whiterosed: bad message", line); return; }
        if (msg.reply !== undefined) {
            const done = root.pending[msg.reply];
            delete root.pending[msg.reply];
            if (done) done(msg.value, msg.error);
            return;
        }
        switch (msg.event) {
        case "settings": root.settings = msg.values; break;
        case "setting": root.apply(msg.key, msg.value); break;
        case "state": root.state = msg; break;
        case "lock": root.lockRequested(); break;
        case "apps": root.appsEvent(msg); break;
        case "fingerprint": root.fingerprintEvent(msg); break;
        case "updates": root.updatesEvent(msg.status); break;
        case "unlock": root.unlockRequested(); break;
        case "report_show": root.reportRequested(msg.from ?? ""); break;
        case "report_snapshots": root.reportSnapshotsEvent(msg.snapshots ?? []); break;
        case "memory_warning": root.memoryWarning(msg); break;
        }
    }

    Socket {
        id: socket
        path: root.socketPath
        connected: true
        parser: SplitParser { onRead: line => root.handle(line) }
        onConnectedChanged: {
            if (connected) return;
            // Replies that will never come.
            const waiting = root.pending;
            root.pending = {};
            for (const id in waiting) waiting[id](undefined, "whiterosed went away");
        }
    }
    // While disconnected (the daemon starting, or restarted), try again each
    // second; nothing runs once connected.
    Timer {
        running: !socket.connected
        repeat: true
        interval: 1000
        onTriggered: socket.connected = true
    }
}
