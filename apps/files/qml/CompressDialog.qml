// Compress…: what's selected into one archive beside it. Zip (opens
// anywhere), 7z (smallest), or tar.gz / tar.xz / tar.zst; zip and 7z can
// have a password (AES-256), and any can be split into parts that fit a
// size (name.zip.001, .002...: Files and 7-Zip put them back together).
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F

Popup {
    id: root
    property var paths: []
    property string dir: ""
    signal made(int job)
    property string format: "zip"
    property real split: 0
    property string error: ""
    readonly property bool canLock: root.format === "zip" || root.format === "7z"

    function show(paths, dir) {
        root.paths = paths;
        root.dir = dir;
        root.error = "";
        pw.text = "";
        const names = paths.map(p => p.substring(p.lastIndexOf("/") + 1));
        // One: its name. Several: what they share, else the folder they're in.
        const folder = dir.substring(dir.lastIndexOf("/") + 1) || "Archive";
        name.text = names.length === 1 ? names[0].replace(/\.[^./]+$/, "") || names[0] : F.groupName(names) === "New folder" ? folder : F.groupName(names);
        root.open();
    }
    function go() {
        const n = name.text.trim();
        if (n === "") { root.error = "Give it a name"; return }
        if (n.includes("/")) { root.error = "A name can't have / in it"; return }
        const dest = root.dir + "/" + n + "." + root.format;
        if (Fs.exists(dest) || Fs.exists(dest + ".001")) { root.error = "There's already one called that here"; return }
        const id = Ops.compress(JSON.stringify(root.paths), dest, root.canLock ? pw.text : "", root.split);
        if (id < 0) { root.error = "Nothing to compress"; return }
        root.made(id);
        root.close();
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    width: 420
    modal: true
    padding: 20
    background: Rectangle { color: Theme.surface1; radius: 16; border { width: 1; color: Theme.surface2 } }
    Overlay.modal: Rectangle { color: Qt.rgba(0, 0, 0, 0.35) }
    onOpened: { name.focusField(); name.selectAll() }

    component Label: Text {
        width: 90
        anchors.verticalCenter: parent.verticalCenter
        color: Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(12) }
    }

    contentItem: Column {
        spacing: 14
        Text {
            text: "Compress " + (root.paths.length === 1 ? "“" + root.paths[0].substring(root.paths[0].lastIndexOf("/") + 1) + "”" : root.paths.length + " items")
            width: parent.width
            elide: Text.ElideMiddle
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
        }
        Row {
            spacing: 10
            Label { text: "Name" }
            Field { id: name; width: 210; onAccepted: Qt.callLater(root.go); onTextChanged: root.error = "" }
            Text { anchors.verticalCenter: parent.verticalCenter; text: "." + root.format; color: Theme.faint; font { family: Theme.mono; pointSize: Theme.pt(12) } }
        }
        Row {
            spacing: 10
            Label { text: "Kind" }
            Choice {
                value: root.format
                options: [{ label: "Zip: opens anywhere", value: "zip" }, { label: "7z: smallest", value: "7z" },
                          { label: "tar.gz", value: "tar.gz" }, { label: "tar.xz: small", value: "tar.xz" }, { label: "tar.zst: quick", value: "tar.zst" }]
                onPicked: v => root.format = v
            }
        }
        Row {
            visible: root.canLock
            spacing: 10
            Label { text: "Password" }
            Field { id: pw; width: 250; secret: true; placeholder: "None"; onAccepted: Qt.callLater(root.go) }
        }
        Row {
            spacing: 10
            Label { text: "Split" }
            Choice {
                value: root.split
                options: [{ label: "Don't split", value: 0 }, { label: "Parts of 100 MB", value: 100e6 },
                          { label: "Parts of 700 MB (a CD)", value: 700e6 }, { label: "Parts of 4 GB (FAT32 drives)", value: 4294967295 }]
                onPicked: v => root.split = v
            }
        }
        Text {
            visible: root.error !== "" || (root.canLock && pw.text !== "")
            width: parent.width
            wrapMode: Text.Wrap
            text: root.error !== "" ? root.error : "Names of files inside a zip can still be seen without the password; in a 7z they're hidden too."
            color: root.error !== "" ? Theme.rose : Theme.faint
            font { family: Theme.font; pointSize: Theme.pt(11) }
        }
        Row {
            anchors.right: parent.right
            spacing: 8
            Button { text: "Cancel"; kind: "ghost"; onClicked: root.close() }
            Button { text: "Compress"; kind: "primary"; onClicked: root.go() }
        }
    }
}
