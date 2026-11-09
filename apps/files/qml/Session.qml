// Every open Files window in this process, and what's kept between runs:
// the tabs of all windows (saved a moment after anything changes, and as
// the last window closes) and the tabs closed recently, to reopen.
//
// Only a Files started without folders owns the saved session (restores it,
// keeps it up to date); one opened on a folder from elsewhere leaves it be.
pragma Singleton
import QtQuick
import WhiteRose.Files

QtObject {
    id: session
    property bool owner: false
    // Hidden files shown, everywhere.
    property bool hidden: Fs.showsHidden()
    function setHidden(on) { Fs.setShowsHidden(on); session.hidden = on }
    property var windows: []
    // Closed tabs, newest last: { tab: snapshot, index }.
    property var closed: []

    // Starts the first window; returns its tabs ({tabs, current}).
    function start() {
        const args = JSON.parse(Fs.arguments());
        if (args.length > 0) return { tabs: args.map(a => ({ path: a })), current: 0 };
        session.owner = true;
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
