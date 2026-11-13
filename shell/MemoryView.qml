// Island view: memory is running out and one app looks like the cause
// (whiterosed's memory watcher, cmd/whiterosed/memory.go). Close asks it to
// close (and makes it after 3 s); Not now leaves it alone for 10 minutes.
// Or, at critical, the watcher already closed it: what and why, with OK and
// Report (its crash record, in the report flow).
// Protected apps (Settings, and the terminal) are never named here.
import QtQuick
import Quickshell
import qs
import qs.settings
import "Memory.js" as Mem

Item {
    id: root
    implicitWidth: 540
    implicitHeight: body.implicitHeight + 36

    readonly property var w: ShellState.memoryWarning
    readonly property string appId: w?.app?.id ?? ""
    readonly property var entry: appId ? (DesktopEntries.byId(appId) ?? DesktopEntries.heuristicLookup(appId)) : null
    readonly property string name: entry?.name || Mem.appName(appId)
    // Closed by the watcher (memory_closed), or a warning (memory_warning).
    readonly property bool closed: w?.event === "memory_closed"
    readonly property var text: closed ? Mem.closedText(w, name) : Mem.warningText(w, name)
    property string error: ""

    function act(op) {
        error = "";
        Daemon.request({ op: op, name: w?.app?.unit ?? "" }, (v, err) => {
            if (!root) return;
            if (err) { root.error = err; return; }
            ShellState.memoryWarning = null;
            ShellState.close();
        });
    }

    Row {
        id: body
        x: 22; y: 18
        width: parent.width - 44
        spacing: 14
        Rectangle {
            width: 40; height: 40; radius: 12
            color: Theme.tint(0.14)
            Icon { anchors.centerIn: parent; name: "gauge"; size: 18; color: Theme.rose }
        }
        Column {
            width: parent.width - 54
            spacing: 12
            Column {
                width: parent.width
                spacing: 3
                Text {
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: root.text[0]
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(13.5); weight: Font.DemiBold }
                }
                Text {
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: root.text[1]
                    color: Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(11.5) }
                }
                Text {
                    visible: root.error !== ""
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: root.error
                    color: Theme.rose
                    font { family: Theme.font; pointSize: Theme.pt(11.5) }
                }
            }
            Row {
                visible: !root.closed
                spacing: 8
                Button {
                    text: "Close " + root.name
                    kind: "primary"
                    onClicked: root.act("memory_close")
                }
                Button {
                    text: "Not now"
                    kind: "ghost"
                    onClicked: root.act("memory_snooze")
                }
            }
            // Closed: report it like a crash (its record), or carry on.
            Row {
                visible: root.closed
                spacing: 8
                Button {
                    text: "OK"
                    kind: "primary"
                    onClicked: root.act("memory_seen")
                }
                Button {
                    visible: (root.w?.record ?? "") !== ""
                    text: "Report"
                    kind: "ghost"
                    onClicked: {
                        const rec = root.w.record;
                        Daemon.request({ op: "memory_seen" }, () => {});
                        ShellState.memoryWarning = null;
                        ShellState.report(rec);
                    }
                }
            }
        }
    }
}
