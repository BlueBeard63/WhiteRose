// Files › Views & lists: how a folder looks until you choose for it (its own
// view, set in Files, always wins), item size, where folders go, the list's
// columns, and folder sizes.
import QtQuick
import Quickshell.Io
import qs
import qs.settings

Column {
    id: page
    spacing: 16
    readonly property var dv: Daemon.setting("files.defaultView", ({ mode: "grid", sortBy: "name", descending: false, group: "" }))
    function setView(changes) { Daemon.set("files.defaultView", Object.assign({}, page.dv, changes)) }
    readonly property var columns: Daemon.setting("files.columns", ["size", "modified", "kind"])
    function toggleColumn(key) {
        const on = page.columns.includes(key);
        Daemon.set("files.columns", ["size", "modified", "created", "kind"].filter(k => k === key ? !on : page.columns.includes(k)));
    }
    readonly property real zoom: Daemon.setting("files.zoom", 1)

    PageHeader { title: "Views & lists"; subtitle: "Files › how folders look until you choose for one." }

    Section { text: "Folders without a view of their own" }
    Card {
        SettingRow {
            title: "View"
            Choice {
                value: page.dv.mode
                options: [{ label: "Grid", value: "grid" }, { label: "List", value: "list" }, { label: "Columns", value: "columns" },
                          { label: "Cards", value: "cards" }, { label: "Disk usage", value: "usage" }]
                onPicked: v => page.setView({ mode: v })
            }
        }
        SettingRow {
            title: "Sort by"
            Row {
                spacing: 8
                Choice {
                    value: page.dv.sortBy
                    options: [{ label: "Name", value: "name" }, { label: "Size", value: "size" }, { label: "Date modified", value: "modified" },
                              { label: "Date created", value: "created" }, { label: "Kind", value: "kind" }]
                    onPicked: v => page.setView({ sortBy: v, descending: !["name", "kind"].includes(v) })
                }
                Segmented {
                    value: page.dv.descending
                    options: [{ label: "A–Z", value: false }, { label: "Z–A", value: true }]
                    onPicked: v => page.setView({ descending: v })
                }
            }
        }
        SettingRow {
            title: "Group by"
            Choice {
                value: page.dv.group
                options: [{ label: "Nothing", value: "" }, { label: "Kind", value: "kind" }, { label: "Date modified", value: "modified" },
                          { label: "Size", value: "size" }, { label: "Name", value: "name" }, { label: "Tag", value: "tag" }]
                onPicked: v => page.setView({ group: v })
            }
        }
        SettingRow {
            title: "Pick views to suit each folder"
            subtitle: "Pictures open as a grid of thumbnails, code projects as a list"
            Toggle { checked: Daemon.setting("files.adaptive", true); onToggled: on => Daemon.set("files.adaptive", on) }
        }
    }

    Section { text: "Everywhere" }
    Card {
        SettingRow {
            title: "Item size"
            subtitle: Math.round(page.zoom * 100) + "% (Ctrl+= and Ctrl+- in Files)"
            Slider {
                implicitWidth: 200
                value: (page.zoom - 0.75) / 0.85
                onMoved: v => Daemon.set("files.zoom", Math.round((0.75 + v * 0.85) * 20) / 20)
            }
        }
        SettingRow {
            title: "Folders"
            Segmented {
                value: Daemon.setting("files.foldersAt", "first")
                options: [{ label: "First", value: "first" }, { label: "Mixed in", value: "mixed" }, { label: "Last", value: "last" }]
                onPicked: v => Daemon.set("files.foldersAt", v)
            }
        }
        SettingRow {
            title: "Folder sizes"
            subtitle: "Worked out in the background for lists and cards; off saves battery on big drives"
            Toggle { checked: Daemon.setting("files.folderSizes", true); onToggled: on => Daemon.set("files.folderSizes", on) }
        }
    }

    Section { text: "List columns" }
    Card {
        Repeater {
            model: [{ key: "size", label: "Size" }, { key: "modified", label: "Date modified" }, { key: "created", label: "Date created" }, { key: "kind", label: "Kind" }]
            delegate: SettingRow {
                required property var modelData
                title: modelData.label
                Toggle { checked: page.columns.includes(modelData.key); onToggled: page.toggleColumn(modelData.key) }
            }
        }
        SettingRow {
            title: "Column widths"
            subtitle: "Drag a heading's edge in Files to change one"
            Button {
                text: "Reset widths"
                enabled: Object.keys(Daemon.setting("files.widths", ({}))).length > 0
                onClicked: Daemon.set("files.widths", {})
            }
        }
    }

    Section { text: "Folders' own views" }
    Card {
        SettingRow {
            title: "Forget every folder's own view"
            subtitle: "They all go back to the view above (colours and icons stay)"
            Button { text: "Forget…"; kind: "danger"; onClicked: forget.open() }
        }
    }
    Dialog {
        id: forget
        title: "Forget every folder's view?"
        subtitle: "They'll all use the view at the top of this page."
        footer: [
            Item { width: parent.width - forgetCancel.width - forgetGo.width - 16; height: 1 },
            Button { id: forgetCancel; text: "Cancel"; onClicked: forget.close() },
            Button { id: forgetGo; text: "Forget"; kind: "danger"; onClicked: { forgetter.running = true; forget.close() } }
        ]
    }
    // (The views live in Files' own state file; Files reads it fresh each time.)
    Process {
        id: forgetter
        command: ["sh", "-c", "rm -f \"${XDG_STATE_HOME:-$HOME/.local/state}/whiterose/files/views.json\""]
    }
}
