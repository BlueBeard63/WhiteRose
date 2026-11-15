// Files › Search: what the search index covers (home always; drives if
// on; folders left out), and making it again. whiterosed makes it in the
// background, at the lowest priority (`whiterose-files --index`): when
// asked, when what's indexed changes, and hourly when plugged in.
import QtQuick
import Quickshell
import qs
import qs.settings
import "../../../Wallpapers.js" as W

Column {
    id: page
    spacing: 16
    readonly property string home: Quickshell.env("HOME")
    readonly property var exclude: Daemon.setting("files.indexExclude", [])
    readonly property real asked: Daemon.setting("files.reindex", 0)
    // whiterosed's files_index: {made (unix s; 0 none), progress (while
    // it's being made: {started (ms), count, expected}) or null}.
    property var index: ({ made: 0, progress: null })
    readonly property bool busy: index.progress !== null || asked > index.made * 1000
    function ago(s) {
        if (!s) return "Not made yet";
        const d = new Date(s * 1000);
        const m = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"][d.getMonth()];
        return "Made " + d.getDate() + " " + m + ", " + String(d.getHours()).padStart(2, "0") + ":" + String(d.getMinutes()).padStart(2, "0");
    }
    // As Files shows it: "Being made · 42% · about 2 min left" (the share
    // from the last index's size; the first time, a count).
    function making(p) {
        if (!p || !p.count) return "Being made…";
        if (!p.expected) return "Being made · " + p.count.toLocaleString() + " items so far";
        const done = Math.min(0.99, p.count / p.expected);
        const took = (Date.now() - p.started) / 1000;
        const pct = Math.round(done * 100) + "%";
        if (took < 1) return "Being made · " + pct;
        const left = took / done * (1 - done);
        const t = left < 45 ? "under a minute" : left < 3600 ? "about " + Math.max(1, Math.round(left / 60)) + " min" : "about " + Math.round(left / 360) / 10 + " h";
        return "Being made · " + pct + " · " + t + " left";
    }
    function refresh() { Daemon.request({ op: "files_index" }, (v, err) => { if (!err && v) page.index = v }) }
    Component.onCompleted: refresh()
    // While it's being made (or asked for and not started yet): follow it.
    Timer { running: page.busy && page.visible; interval: 1000; repeat: true; onTriggered: page.refresh() }

    PageHeader { title: "Search"; subtitle: "Files › what searching Home and Everywhere looks through." }

    Card {
        SettingRow {
            title: "Search index"
            subtitle: page.busy ? page.making(page.index.progress) : page.ago(page.index.made)
            Button {
                text: "Make again"
                enabled: !page.busy
                onClicked: { Daemon.set("files.reindex", Date.now()); page.refresh() }
            }
        }
        SettingRow {
            title: "Drives too"
            subtitle: "USB sticks and other drives, as well as your home folder"
            Toggle { checked: Daemon.setting("files.indexDrives", true); onToggled: on => Daemon.set("files.indexDrives", on) }
        }
    }
    Text {
        width: parent.width
        wrapMode: Text.Wrap
        text: "“This folder” searches always look at the folder as it is. Hidden folders, node_modules and Go's module cache are never indexed. The index is kept in your cache folder, readable only by you."
        color: Theme.faint
        font { family: Theme.font; pointSize: Theme.pt(11.5) }
    }

    Section { text: "Left out" }
    Card {
        SettingRow {
            visible: page.exclude.length === 0
            title: "Nothing left out"
            subtitle: "Folders added here aren't indexed (searching them directly still works)"
        }
        Repeater {
            model: page.exclude
            delegate: SettingRow {
                required property string modelData
                title: modelData
                IconButton {
                    width: 28; height: 28; icon: "x"; iconSize: 12
                    onClicked: Daemon.set("files.indexExclude", page.exclude.filter(p => p !== modelData))
                }
            }
        }
        SettingRow {
            title: ""
            Button { text: "Leave a folder out…"; onClicked: folderDialog.open() }
        }
    }

    Section { text: "Privacy" }
    Card {
        SettingRow {
            divider: false
            title: "Encrypt the index and history"
            subtitle: "The search index (every file name), the folders you've visited and the log of what was copied, moved and deleted, sealed with a key kept in your keyring. Not needed on an encrypted disk; searching then reads the index into memory."
            Toggle { checked: Daemon.setting("files.encrypt", false); onToggled: on => Daemon.set("files.encrypt", on) }
        }
    }

    FilePicker {
        id: folderDialog
        title: "A folder to leave out of the index"
        folder: page.home
        folders: true
        onPicked: picked => {
            const path = W.tildify(picked, page.home);
            if (!page.exclude.includes(path)) Daemon.set("files.indexExclude", page.exclude.concat([path]));
        }
    }
}
