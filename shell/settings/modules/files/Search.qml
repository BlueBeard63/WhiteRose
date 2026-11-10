// Files › Search: what the search index covers (home always; drives if
// on; folders left out), and making it again. Files makes it in the
// background, at the lowest priority: now if it's open, else next start.
import QtQuick
import QtQuick.Dialogs
import Quickshell
import Quickshell.Io
import qs
import qs.settings
import "../../../Wallpapers.js" as W

Column {
    id: page
    spacing: 16
    readonly property string home: Quickshell.env("HOME")
    readonly property var exclude: Daemon.setting("files.indexExclude", [])
    readonly property real asked: Daemon.setting("files.reindex", 0)
    // When the index was made (unix s; 0 none).
    property real made: 0
    function ago(s) {
        if (!s) return "Not made yet";
        const d = new Date(s * 1000);
        const m = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"][d.getMonth()];
        return "Made " + d.getDate() + " " + m + ", " + String(d.getHours()).padStart(2, "0") + ":" + String(d.getMinutes()).padStart(2, "0");
    }

    Process {
        id: stat
        running: true
        command: ["sh", "-c", "stat -c %Y \"${XDG_CACHE_HOME:-$HOME/.cache}/whiterose/files/index\" 2>/dev/null || echo 0"]
        stdout: StdioCollector { onStreamFinished: page.made = parseInt(text) || 0 }
    }
    // Asked for again: look again in a while (Files makes it in the background).
    Timer { running: page.asked > page.made * 1000; interval: 3000; repeat: true; onTriggered: stat.running = true }

    PageHeader { title: "Search"; subtitle: "Files › what searching Home and Everywhere looks through." }

    Card {
        SettingRow {
            title: "Search index"
            subtitle: page.asked > page.made * 1000 ? "Being made again…" : page.ago(page.made)
            Button {
                text: "Make again"
                enabled: !(page.asked > page.made * 1000)
                onClicked: Daemon.set("files.reindex", Date.now())
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

    FolderDialog {
        id: folderDialog
        title: "A folder to leave out of the index"
        currentFolder: "file://" + page.home
        onAccepted: {
            const path = W.tildify(decodeURIComponent(String(selectedFolder).replace(/^file:\/\//, "")), page.home);
            if (!page.exclude.includes(path)) Daemon.set("files.indexExclude", page.exclude.concat([path]));
        }
    }
}
