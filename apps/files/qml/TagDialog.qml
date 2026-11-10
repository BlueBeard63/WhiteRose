// A new tag (a name and a colour; the files it's for get it straight away),
// or a new name for one (changed on every file that has it).
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files

Popup {
    id: root
    // "new" or "rename".
    property string mode: "new"
    property string from: ""
    property var paths: []
    property string colour: "rose"
    property string error: ""
    signal done(string name)

    function newTag(paths) {
        root.mode = "new"; root.paths = paths; root.from = "";
        root.colour = Session.freshColour(); root.error = "";
        nameField.text = "";
        root.open();
    }
    function rename(name) {
        root.mode = "rename"; root.from = name; root.paths = [];
        root.colour = Session.tagDefs.find(t => t.name === name)?.colour ?? "grey"; root.error = "";
        nameField.text = name;
        root.open();
    }
    function save() {
        const c = JSON.parse(Fs.checkTag(nameField.text));
        if (!c.ok) { root.error = c.error; return }
        const name = c.name;
        const clash = Session.tagDefs.some(t => t.name.toLowerCase() === name.toLowerCase() && t.name !== root.from);
        if (root.mode === "new") {
            if (!clash) Session.defineTag(name, root.colour);
            if (root.paths.length > 0) {
                const r = JSON.parse(Fs.setTag(JSON.stringify(root.paths), name, true));
                if (r.failed.length > 0) { root.error = r.failed[0][1]; return }
            }
        } else {
            if (clash) { root.error = "There's already a tag called " + name; return }
            if (name !== root.from) {
                const r = JSON.parse(Fs.renameTag(root.from, name));
                if (!r.ok) root.error = r.error;
            }
            Session.setTagDefs(Session.tagDefs.map(t => t.name === root.from ? { name: name, colour: root.colour } : t));
        }
        // Once Enter is handled (closing during it can crash Qt).
        Qt.callLater(() => { root.close(); root.done(name) });
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    width: 360
    modal: true
    focus: true
    padding: 20
    background: Rectangle { color: Theme.surface1; radius: 16; border { width: 1; color: Theme.surface2 } }
    Overlay.modal: Rectangle { color: Qt.rgba(0, 0, 0, 0.35) }
    onOpened: { nameField.focusField(); nameField.selectAll() }

    contentItem: Column {
        spacing: 14
        Text {
            text: root.mode === "new" ? (root.paths.length > 0 ? "New tag for " + (root.paths.length === 1 ? "this" : root.paths.length + " items") : "New tag") : "Rename tag"
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
        }
        Field {
            id: nameField
            width: 320
            placeholder: "Name"
            error: root.error !== ""
            onTextChanged: root.error = ""
            onAccepted: root.save()
            Keys.onEscapePressed: root.close()
        }
        Flow {
            width: 320
            spacing: 6
            Repeater {
                model: Object.keys(Session.palette)
                delegate: Rectangle {
                    required property string modelData
                    width: 24; height: 24; radius: 12
                    color: Session.palette[modelData]
                    border { width: root.colour === modelData ? 2 : 0; color: Theme.text }
                    MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: root.colour = modelData }
                }
            }
        }
        Text {
            visible: root.error !== "" || root.mode === "rename"
            width: 320
            wrapMode: Text.Wrap
            text: root.error !== "" ? root.error : "Changed on every file that has it."
            color: root.error !== "" ? Theme.rose : Theme.faint
            font { family: Theme.font; pointSize: Theme.pt(11.5) }
        }
        Row {
            anchors.right: parent.right
            spacing: 8
            Button { text: "Cancel"; kind: "ghost"; onClicked: root.close() }
            Button { text: root.mode === "new" ? "Add" : "Rename"; kind: "primary"; onClicked: root.save() }
        }
    }
}
