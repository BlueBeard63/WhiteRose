// Files › Toolbar & menus: extra toolbar buttons, the built-in right-click
// entries to leave out, and your own actions (a command run on what's
// chosen: %f the paths, %n the names, %d the folder). All edited here, in
// place; Files picks changes up as they're made.
import QtQuick
import qs
import qs.settings
import "FilesActions.js" as A

Column {
    id: page
    spacing: 16

    readonly property var toolbar: Daemon.setting("files.toolbar", [])
    readonly property var hidden: Daemon.setting("files.menuHide", [])
    readonly property var actions: Daemon.setting("files.actions", [])
    property string error: ""

    function setToolbar(id, on) {
        // In the order of the list, so buttons always sit the same way round.
        const want = A.toolbar.map(t => t.id).filter(x => x === id ? on : page.toolbar.includes(x));
        Daemon.set("files.toolbar", want);
    }
    function setShown(id, on) {
        Daemon.set("files.menuHide", on ? page.hidden.filter(x => x !== id) : page.hidden.concat([id]));
    }
    function saveActions(list) {
        page.error = "";
        Daemon.request({ op: "set", key: "files.actions", value: list }, (v, err) => { if (err) page.error = String(err) });
    }
    function editAction(i, change) {
        const next = page.actions.map((a, j) => j === i ? Object.assign({}, a, change) : a);
        // (Saved once it has what it needs; the daemon checks the rest.)
        if (next.every(a => a.name.trim() !== "" && a.command.trim() !== "")) page.saveActions(next);
    }

    PageHeader { title: "Toolbar & menus"; subtitle: "What's in Files' toolbar and right-click menu, and your own actions." }

    Section { text: "Toolbar buttons" }
    Card {
        Repeater {
            model: A.toolbar
            delegate: SettingRow {
                required property var modelData
                required property int index
                divider: index > 0
                title: modelData.label
                Icon { anchors.verticalCenter: parent.verticalCenter; name: modelData.icon; size: 15; color: Theme.muted }
                Toggle { checked: page.toolbar.includes(modelData.id); onToggled: on => page.setToolbar(modelData.id, on) }
            }
        }
    }

    Section { text: "Right-click menu" }
    Card {
        Repeater {
            model: A.menu
            delegate: SettingRow {
                required property var modelData
                required property int index
                divider: index > 0
                title: modelData.label.replace("…", "") + (modelData.also ? " / " + modelData.also.replace("Copy to ", "").replace("Move to ", "move to ") : "")
                Toggle { checked: !page.hidden.includes(modelData.id); onToggled: on => page.setShown(modelData.id, on) }
            }
        }
    }

    Section { text: "Your actions" }
    Text {
        width: parent.width
        wrapMode: Text.Wrap
        text: "Commands in the right-click menu for what you've chosen. %f is the chosen paths, %n their names, %d the folder (each quoted); with none of these the paths go on the end. For example, “Make smaller”: convert %f -resize 50% small-%n, for files of types jpg, png."
        color: Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(11.5) }
    }
    Repeater {
        model: page.actions
        delegate: Card {
            id: act
            required property var modelData
            required property int index
            SettingRow {
                divider: false
                title: "Name"
                Field { implicitWidth: 260; text: act.modelData.name; onEditingDone: if (text !== act.modelData.name) page.editAction(act.index, { name: text }) }
                IconButton {
                    width: 28; height: 28; icon: "trash"; iconSize: 13
                    onClicked: page.saveActions(page.actions.filter((a, j) => j !== act.index))
                }
            }
            SettingRow {
                title: "Command"
                Field { implicitWidth: 300; mono: true; text: act.modelData.command; onEditingDone: if (text !== act.modelData.command) page.editAction(act.index, { command: text }) }
            }
            SettingRow {
                title: "For"
                Segmented {
                    value: act.modelData.on
                    options: [{ label: "Files", value: "files" }, { label: "Folders", value: "folders" }, { label: "Either", value: "any" }]
                    onPicked: v => page.editAction(act.index, { on: v })
                }
            }
            SettingRow {
                visible: act.modelData.on !== "folders"
                title: "Only these types"
                subtitle: "Extensions, e.g. jpg, png (empty: any)"
                Field { implicitWidth: 200; mono: true; text: act.modelData.types; placeholder: "any"; onEditingDone: if (text !== act.modelData.types) page.editAction(act.index, { types: text }) }
            }
            SettingRow {
                title: "Run in a terminal"
                subtitle: "To see what it says, or answer it"
                Toggle { checked: act.modelData.terminal; onToggled: on => page.editAction(act.index, { terminal: on }) }
            }
        }
    }
    Text {
        visible: page.error !== ""
        width: parent.width
        wrapMode: Text.Wrap
        text: page.error
        color: Theme.rose
        font { family: Theme.font; pointSize: Theme.pt(11.5) }
    }
    Button {
        anchors.verticalCenter: undefined
        text: "Add an action"
        icon: "plus"
        onClicked: page.saveActions(page.actions.concat([{ name: "New action", command: "echo %f", on: "files", types: "", terminal: true }]))
    }
}
