// A picker's strip (`whiterose-files --choose`, choose.rs): above the
// files, what's wanted (the title), which kind of file (when there are
// several), the name to save as, and Cancel / Open · Choose folder · Save.
// Opening a file chooses it; Esc or closing the window cancels. Saving over
// a file asks first, here.
import QtQuick
import WhiteRose.Kit
import WhiteRose.Files

Strip {
    id: root
    // The folder in use.
    property var pane: null
    readonly property var c: Session.choose
    // Saving over this one, if Save is pressed again.
    property string replacing: ""
    property string problem: ""

    visible: !!c
    icon: !c ? "" : c.mode === "save" ? "download" : c.mode === "folder" ? "folder" : "file"
    tone: replacing !== "" ? "warn" : "plain"
    focusOn: c?.mode === "save" ? nameField : null
    Component.onCompleted: if (c) { nameField.text = c.name; open() }
    onClosed: Fs.chooseDone("null")

    // A real folder (not Trash, Recent, a search…): somewhere to save, or
    // to choose.
    readonly property bool inFolder: !!pane && !pane.searching && !pane.special && String(pane.path).startsWith("/")
    readonly property var files: (pane?.selection ?? []).filter(e => !e.dir)
    readonly property var picked: !c ? [] : c.mode === "folder"
        ? ((pane?.selection ?? []).length === 1 && pane.selection[0].dir ? [pane.selection[0].path] : inFolder ? [pane.path] : [])
        : c.mode === "save" ? (inFolder && nameField.text.trim() !== "" ? [pane.path.replace(/\/$/, "") + "/" + nameField.text.trim()] : [])
        : (c.multiple ? files : files.slice(0, 1)).map(e => e.path)

    function done() {
        if (picked.length === 0) return;
        if (c.mode === "save") {
            const name = nameField.text.trim();
            if (name.includes("/")) { problem = "A name can't have a / in it"; return }
            if (Fs.exists(picked[0]) && replacing !== picked[0]) { replacing = picked[0]; return }
        }
        Fs.chooseDone(JSON.stringify(picked));
    }
    // Opening `entry` (double click, Enter): choosing it.
    function openEntry(entry) {
        if (entry.dir) { pane?.go(entry.path); return }
        if (c.mode === "save") { nameField.text = entry.name; replacing = ""; return }
        if (c.mode === "open") Fs.chooseDone(JSON.stringify([entry.path]));
    }

    Text {
        anchors.verticalCenter: parent.verticalCenter
        text: root.replacing !== "" ? "“" + nameField.text.trim() + "” is already here. Replace it?" : (root.c?.title ?? "")
        color: Theme.text
        font { family: Theme.font; pointSize: Theme.pt(12); weight: Font.Medium }
    }
    Choice {
        anchors.verticalCenter: parent.verticalCenter
        visible: (root.c?.filters ?? []).length > 1
        label: "Kind of file"
        value: Session.chooseFilter
        options: (root.c?.filters ?? []).map((f, i) => ({ label: f.name, value: i }))
        onPicked: v => Session.chooseFilter = v
    }
    Field {
        id: nameField
        anchors.verticalCenter: parent.verticalCenter
        visible: root.c?.mode === "save"
        label: "Name"
        width: 260; height: 30
        error: root.problem !== ""
        onTextChanged: { root.replacing = ""; root.problem = "" }
        onAccepted: root.done()
        Keys.onEscapePressed: root.close()
    }
    Text {
        anchors.verticalCenter: parent.verticalCenter
        visible: root.problem !== ""
        text: root.problem
        color: Theme.rose
        font { family: Theme.font; pointSize: Theme.pt(10.5) }
    }
    buttons: [
        Button { text: "Cancel"; kind: "ghost"; onClicked: root.close() },
        Button {
            text: root.replacing !== "" ? "Replace" : root.c?.mode === "save" ? "Save" : root.c?.mode === "folder" ? "Choose folder" : "Open"
            kind: root.replacing !== "" ? "danger" : "primary"
            enabled: root.picked.length > 0
            onClicked: root.done()
        }
    ]
}
