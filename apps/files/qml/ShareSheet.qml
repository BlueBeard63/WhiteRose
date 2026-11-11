// The Sheet's Share tab: what's selected, made ready to send. Compress
// into one archive beside it: Zip (opens anywhere), 7z (smallest), or
// tar.gz / tar.xz / tar.zst; zip and 7z can have a password (AES-256),
// and any can be split into parts that fit a size (name.zip.001, .002...:
// Files and 7-Zip put them back together). And its path, copied as text,
// for a terminal, or as a link.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F

Column {
    id: root
    property var pane: null
    signal made(int job)
    signal said(string text)

    readonly property var sel: root.pane?.selection ?? []
    readonly property var paths: root.sel.map(e => e.path)
    readonly property string dir: root.pane?.path ?? ""
    readonly property bool can: root.paths.length > 0 && !(root.pane?.special ?? true) && !(root.pane?.inArchive ?? false) && !(root.pane?.readOnly ?? false)
    property string format: "zip"
    property real split: 0
    property string error: ""
    readonly property bool canLock: root.format === "zip" || root.format === "7z"
    readonly property bool typing: name.editing || pw.editing

    // A name from what's selected: one, its name; several, what they share,
    // else the folder they're in.
    function suggest() {
        root.error = "";
        pw.text = "";
        const names = root.paths.map(p => p.substring(p.lastIndexOf("/") + 1));
        const folder = root.dir.substring(root.dir.lastIndexOf("/") + 1) || "Archive";
        name.text = names.length === 0 ? "" : names.length === 1 ? names[0].replace(/\.[^./]+$/, "") || names[0] : F.groupName(names) === "New folder" ? folder : F.groupName(names);
    }
    onPathsChanged: suggest()
    function go() {
        const n = name.text.trim();
        if (n === "") { root.error = "Give it a name"; return }
        if (n.includes("/")) { root.error = "A name can't have / in it"; return }
        const dest = root.dir + "/" + n + "." + root.format;
        if (Fs.exists(dest) || Fs.exists(dest + ".001")) { root.error = "There's already one called that here"; return }
        const id = Ops.compress(JSON.stringify(root.paths), dest, root.canLock ? pw.text : "", root.split);
        if (id < 0) { root.error = "Nothing to compress"; return }
        root.made(id);
    }
    function copy(text, what) { Ops.setClipboardText(text); root.said(what) }
    function quote(p) { return "'" + p.replace(/'/g, "'\\''") + "'" }

    spacing: 12

    component Label: Text {
        color: Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(10.5) }
    }
    // A row to click: copies something.
    component CopyRow: Rectangle {
        id: cr
        property string text
        property string icon
        signal clicked()
        width: parent.width; height: 32; radius: 8
        color: crMouse.containsMouse ? Theme.surface1 : "transparent"
        Icon { x: 8; anchors.verticalCenter: parent.verticalCenter; name: cr.icon; size: 13; color: Theme.muted }
        Text { x: 30; anchors.verticalCenter: parent.verticalCenter; text: cr.text; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(12) } }
        MouseArea { id: crMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: cr.clicked() }
    }

    Text {
        width: parent.width
        elide: Text.ElideMiddle
        text: root.paths.length === 0 ? "Share" : root.paths.length === 1 ? "Share “" + root.paths[0].substring(root.paths[0].lastIndexOf("/") + 1) + "”" : "Share " + root.paths.length + " items"
        color: Theme.text
        font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
    }
    Text {
        visible: root.paths.length === 0
        width: parent.width
        wrapMode: Text.Wrap
        text: "Choose what to share in the list."
        color: Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(12) }
    }

    // ---- copy where it is ----
    Column {
        visible: root.paths.length > 0
        width: parent.width
        CopyRow { text: root.paths.length === 1 ? "Copy its path" : "Copy their paths"; icon: "copy"; onClicked: root.copy(root.paths.join("\n"), "Copied") }
        CopyRow { text: "Copy for a terminal (quoted)"; icon: "terminal"; onClicked: root.copy(root.paths.map(root.quote).join(" "), "Copied, quoted") }
        CopyRow { text: "Copy as a link (file://)"; icon: "link"; onClicked: root.copy(root.paths.map(p => "file://" + encodeURI(p)).join("\n"), "Copied as links") }
    }

    // ---- compress ----
    Column {
        visible: root.can
        width: parent.width
        spacing: 10
        Text { text: "COMPRESS"; color: Theme.faint; font { family: Theme.font; pointSize: Theme.pt(10); weight: Font.Medium; letterSpacing: 0.6 } }
        Column {
            width: parent.width
            spacing: 4
            Label { text: "Name" }
            Item {
                width: parent.width; height: 34
                Field { id: name; anchors { left: parent.left; right: ext.left; rightMargin: 6 } height: 34; onAccepted: Qt.callLater(root.go); onTextChanged: root.error = "" }
                Text { id: ext; anchors { right: parent.right; verticalCenter: parent.verticalCenter } text: "." + root.format; color: Theme.faint; font { family: Theme.mono; pointSize: Theme.pt(11.5) } }
            }
        }
        Item {
            width: parent.width; height: 32
            Text { anchors.verticalCenter: parent.verticalCenter; text: "Kind"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(12) } }
            Choice {
                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                value: root.format
                options: [{ label: "Zip: opens anywhere", value: "zip" }, { label: "7z: smallest", value: "7z" },
                          { label: "tar.gz", value: "tar.gz" }, { label: "tar.xz: small", value: "tar.xz" }, { label: "tar.zst: quick", value: "tar.zst" }]
                onPicked: v => root.format = v
            }
        }
        Item {
            width: parent.width; height: 32
            Text { anchors.verticalCenter: parent.verticalCenter; text: "Split"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(12) } }
            Choice {
                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                value: root.split
                options: [{ label: "Don't split", value: 0 }, { label: "Parts of 100 MB", value: 100e6 },
                          { label: "Parts of 700 MB (a CD)", value: 700e6 }, { label: "Parts of 4 GB (FAT32)", value: 4294967295 }]
                onPicked: v => root.split = v
            }
        }
        Column {
            visible: root.canLock
            width: parent.width
            spacing: 4
            Label { text: "Password" }
            Field { id: pw; width: parent.width; height: 34; secret: true; placeholder: "None"; onAccepted: Qt.callLater(root.go) }
        }
        Text {
            visible: root.error !== "" || (root.canLock && pw.text !== "")
            width: parent.width
            wrapMode: Text.Wrap
            text: root.error !== "" ? root.error : "Names of files inside a zip can still be seen without the password; in a 7z they're hidden too."
            color: root.error !== "" ? Theme.rose : Theme.faint
            font { family: Theme.font; pointSize: Theme.pt(10.5) }
        }
        Button { anchors.verticalCenter: undefined; text: "Compress"; icon: "package"; kind: "primary"; onClicked: root.go() }
    }
}
