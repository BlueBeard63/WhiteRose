// The desktop's notification server (org.freedesktop.Notifications): every
// notification apps send lands here, is listed in the notifications panel
// until dismissed, and pops up briefly unless Do Not Disturb is on (critical
// ones always pop up, and stay until dismissed).
pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Services.Notifications
import "Notifications.js" as N

Singleton {
    id: root

    // Newest first.
    readonly property var list: server.trackedNotifications.values.slice().reverse()
    readonly property int count: list.length
    // Something arrived since the panel was last opened (the bell's dot).
    property bool unread: false
    // When each one arrived (the protocol has no timestamp), by id.
    property var arrivedAt: ({})
    // Ids currently popped up; the popups are whichever of these still exist.
    property var popupIds: []
    readonly property var popups: list.filter(n => popupIds.includes(n.id)).slice(0, 3)

    function arrived(n) { return arrivedAt[n.id] ?? Date.now() }

    // Notifications kept across a shell reload have no arrival time: count
    // from now rather than showing "now" forever.
    Component.onCompleted: {
        const t = Date.now();
        for (const n of server.trackedNotifications.values) arrivedAt[n.id] = arrivedAt[n.id] ?? t;
        arrivedAtChanged();
    }

    function hidePopup(n) {
        popupIds = popupIds.filter(id => id !== n.id);
        // Transient notifications aren't kept once they've been seen.
        if (n.transient) n.expire();
    }

    function dismiss(n) { n.dismiss() }
    function clearAll() { for (const n of list) n.dismiss() }

    // Clicking a notification: its default action if it has one, else bring
    // the app forward. Either way it's done with (unless the app keeps it).
    function activate(n) {
        const def = n.actions.find(a => a.identifier === "default");
        if (def) { def.invoke(); return; }
        if (n.desktopEntry) Compositor.focusApp(n.desktopEntry);
        if (!n.resident) n.dismiss();
    }

    // Opening the panel shows everything: popups go, the dot goes.
    Connections {
        target: ShellState
        function onPanelChanged() {
            if (ShellState.panel !== "notifications") return;
            root.unread = false;
            root.popupIds = [];
        }
    }

    NotificationServer {
        id: server
        keepOnReload: true
        bodySupported: true
        actionsSupported: true
        imageSupported: true
        persistenceSupported: true
        onNotification: n => {
            n.tracked = true;
            root.arrivedAt[n.id] = Date.now();
            root.arrivedAtChanged();
            const panelOpen = ShellState.panel === "notifications";
            const muted = Daemon.setting("notifications.muted", []).includes(n.appName);
            if (!panelOpen && !muted) root.unread = true;
            if (N.shouldPopup(n.urgency, QuickState.doNotDisturb, panelOpen, muted))
                root.popupIds = [n.id, ...root.popupIds];
            // Settings → Notifications lists every app that has notified.
            const apps = Daemon.setting("notifications.apps", []);
            if (n.appName && apps[0] !== n.appName) Daemon.set("notifications.apps", N.rememberApp(apps, n.appName));
        }
    }
}
