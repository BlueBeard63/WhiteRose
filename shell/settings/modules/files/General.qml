// Files › General: where Files starts, how a click opens things, the Trash,
// and what shows.
import QtQuick
import qs
import qs.settings

Column {
    id: page
    spacing: 16
    readonly property string startIn: Daemon.setting("files.startIn", "last")

    PageHeader { title: "General"; subtitle: "Files › how it starts and how things open." }

    Section { text: "Starting" }
    Card {
        SettingRow {
            title: "Open with"
            subtitle: page.startIn === "last" ? "The tabs and windows you had last time" : page.startIn === "home" ? "Your home folder"
                : page.startIn === "start" ? "The Start page: workspaces, places, recent files, drives" : "The folder below"
            Choice {
                value: page.startIn
                options: [{ label: "Last tabs", value: "last" }, { label: "Home", value: "home" }, { label: "Start page", value: "start" }, { label: "A folder", value: "folder" }]
                onPicked: v => Daemon.set("files.startIn", v)
            }
        }
        SettingRow {
            visible: page.startIn === "folder"
            title: "Folder"
            subtitle: "~/ for your home folder"
            Field {
                implicitWidth: 260
                mono: true
                placeholder: "~/Projects"
                text: Daemon.setting("files.startFolder", "")
                onAccepted: Daemon.set("files.startFolder", text.trim())
                onEditingDone: Daemon.set("files.startFolder", text.trim())
            }
        }
    }

    Section { text: "Opening" }
    Card {
        SettingRow {
            title: "Open items with"
            subtitle: Daemon.setting("files.openWith", "double") === "single" ? "One click; Ctrl or Shift click to select" : "Two clicks; one click selects"
            Segmented {
                value: Daemon.setting("files.openWith", "double")
                options: [{ label: "Double click", value: "double" }, { label: "Single click", value: "single" }]
                onPicked: v => Daemon.set("files.openWith", v)
            }
        }
        SettingRow {
            title: "Ask before moving to the Trash"
            subtitle: "Delete always goes to the Trash, so things can be brought back"
            Toggle { checked: Daemon.setting("files.confirmTrash", false); onToggled: on => Daemon.set("files.confirmTrash", on) }
        }
        SettingRow {
            title: "Verify copies"
            subtitle: "Read each copied file back from the drive and check it matches (slower; catches failing drives and cables)"
            Toggle { checked: Daemon.setting("files.verifyCopies", false); onToggled: on => Daemon.set("files.verifyCopies", on) }
        }
    }

    Section { text: "Showing" }
    Card {
        SettingRow {
            title: "Hidden files"
            subtitle: "Names starting with a dot (Ctrl+H in Files)"
            Toggle { checked: Daemon.setting("files.hidden", false); onToggled: on => Daemon.set("files.hidden", on) }
        }
        SettingRow {
            title: "Git status"
            subtitle: "Marks on changed files and the branch at the bottom, in git repositories"
            Toggle { checked: Daemon.setting("files.git", true); onToggled: on => Daemon.set("files.git", on) }
        }
        SettingRow {
            title: "Sidebar"
            subtitle: "Places, bookmarks, tags and drives (Ctrl+B in Files; click a heading there to fold it)"
            Toggle { checked: Daemon.setting("files.sidebar", true); onToggled: on => Daemon.set("files.sidebar", on) }
        }
        SettingRow {
            title: "Details panel"
            subtitle: "A preview of the selected file beside the folder (Alt+P)"
            Toggle { checked: Daemon.setting("files.details", false); onToggled: on => Daemon.set("files.details", on) }
        }
    }
}
