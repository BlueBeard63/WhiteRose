// Apps › Memory (docs/plans/memory-protection.md): what each app is using
// now, which apps are kept running when memory runs out, and whether the
// watcher may close the app at fault by itself. whiterosed's memory_status
// (cmd/whiterosed/memory.go) says how things stand; refreshed every 5 s
// while the page is open.
import QtQuick
import Quickshell
import qs
import qs.settings
import "../../../Memory.js" as Mem

Column {
    id: page
    spacing: 16

    property var st: null
    readonly property var kept: Daemon.setting("memory.protected", [])

    function refresh() {
        Daemon.request({ op: "memory_status" }, v => { if (page && v) page.st = v; });
    }
    Component.onCompleted: refresh()
    Timer { interval: 5000; running: true; repeat: true; onTriggered: page.refresh() }

    function nameOf(id) {
        const e = id ? (DesktopEntries.byId(id) ?? DesktopEntries.heuristicLookup(id)) : null;
        return e?.name || Mem.appName(id);
    }
    function setKept(id, on) {
        const list = page.kept.filter(x => x.toLowerCase() !== id.toLowerCase());
        if (on) list.push(id);
        Daemon.set("memory.protected", list);
        Qt.callLater(page.refresh);
    }

    PageHeader {
        title: "Memory"
        subtitle: "When memory runs out, WhiteRose closes the app at fault instead of losing the whole session."
    }

    // How it stands.
    Card {
        SettingRow {
            divider: false
            title: "In use"
            subtitle: page.st
                ? Mem.size(page.st.total - page.st.available) + " of " + Mem.size(page.st.total)
                    + ({ calm: "", tight: " · running low", critical: " · almost none left" })[page.st.level]
                : "Checking…"
            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: 160; height: 6; radius: 3
                color: Theme.surface2
                Rectangle {
                    height: parent.height; radius: 3
                    width: page.st && page.st.total > 0 ? parent.width * Math.min(1, (page.st.total - page.st.available) / page.st.total) : 0
                    color: page.st?.level === "calm" ? Theme.muted : Theme.rose
                }
            }
        }
    }

    Card {
        SettingRow {
            divider: false
            title: "Close apps when memory runs out"
            subtitle: "When everything stalls, the app using the most (or growing steadily) is closed, and you're told which. Apps you keep running are never closed."
            Toggle {
                id: autoToggle
                checked: Daemon.setting("memory.autoClose", true)
                onToggled: on => {
                    Daemon.set("memory.autoClose", on);
                    autoToggle.checked = Qt.binding(() => Daemon.setting("memory.autoClose", true));
                }
            }
        }
    }

    // Each app running now, biggest first.
    Section { text: "Apps running" }
    Card {
        visible: (page.st?.apps ?? []).length === 0
        SettingRow { divider: false; title: page.st ? "No apps running" : "Checking…" }
    }
    Card {
        visible: (page.st?.apps ?? []).length > 0
        Repeater {
            model: page.st?.apps ?? []
            delegate: SettingRow {
                id: appRow
                required property var modelData
                required property int index
                divider: index > 0
                title: page.nameOf(modelData.id)
                subtitle: Mem.size(modelData.bytes)
                    + (modelData.growing ? " · growing steadily" : "")
                    + (modelData.suspect ? " · first to close" : "")
                    + (modelData.kept === "terminal" ? " · your terminal, always kept" : "")
                subtitleColor: modelData.suspect ? Theme.rose : Theme.muted
                Toggle {
                    label: "Keep " + appRow.title + " running"
                    checked: appRow.modelData.kept !== ""
                    enabled: appRow.modelData.kept !== "terminal"
                    onToggled: on => page.setKept(appRow.modelData.id, on)
                }
            }
        }
    }

    // Kept running, but not open now.
    Section { text: "Kept running"; visible: page.notOpen.length > 0 }
    readonly property var notOpen: page.kept.filter(id => !(page.st?.apps ?? []).some(a => a.id.toLowerCase() === id.toLowerCase()))
    Card {
        visible: page.notOpen.length > 0
        Repeater {
            model: page.notOpen
            delegate: SettingRow {
                id: keptRow
                required property string modelData
                required property int index
                divider: index > 0
                title: page.nameOf(modelData)
                subtitle: "Not open"
                Toggle {
                    label: "Keep " + keptRow.title + " running"
                    checked: true
                    onToggled: on => page.setKept(keptRow.modelData, on)
                }
            }
        }
    }

    // What the watcher closed by itself.
    Section { text: "Closed to free memory"; visible: (page.st?.closed ?? []).length > 0 }
    Card {
        visible: (page.st?.closed ?? []).length > 0
        Repeater {
            model: page.st?.closed ?? []
            delegate: SettingRow {
                id: closedRow
                required property var modelData
                required property int index
                divider: index > 0
                title: page.nameOf(modelData.app)
                subtitle: new Date(modelData.time).toLocaleString(Qt.locale(), "d MMM, HH:mm")
                Button {
                    text: "Report"
                    label: "Report " + closedRow.title + " being closed"
                    onClicked: Daemon.request({ op: "report_show", name: closedRow.modelData.id }, () => {})
                }
            }
        }
    }
}
