// An archive wants a password (extracting said so): asked, then tried again.
// Kept in memory for this run only.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files

Popup {
    id: root
    property string archive: ""
    property string dest: ""
    property bool wrong: false
    signal retry(string archive, string dest)

    function ask(archive, dest, wrong) {
        root.archive = archive;
        root.dest = dest;
        root.wrong = wrong;
        pw.text = "";
        root.open();
    }
    function go() {
        if (pw.text === "") return;
        Ops.setArchivePassword(root.archive, pw.text);
        root.close();
        root.retry(root.archive, root.dest);
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    width: 380
    modal: true
    padding: 20
    background: Rectangle { color: Theme.surface1; radius: 16; border { width: 1; color: Theme.surface2 } }
    Overlay.modal: Rectangle { color: Qt.rgba(0, 0, 0, 0.35) }
    onOpened: pw.focusField()

    contentItem: Column {
        spacing: 12
        Text {
            width: parent.width
            text: "“" + root.archive.substring(root.archive.lastIndexOf("/") + 1) + "” needs a password"
            elide: Text.ElideMiddle
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
        }
        Field { id: pw; width: parent.width; secret: true; placeholder: "Password"; error: root.wrong; onAccepted: Qt.callLater(root.go) }
        Text {
            visible: root.wrong
            text: "That password isn't right. Try again."
            color: Theme.rose
            font { family: Theme.font; pointSize: Theme.pt(11.5) }
        }
        Row {
            anchors.right: parent.right
            spacing: 8
            Button { text: "Cancel"; kind: "ghost"; onClicked: root.close() }
            Button { text: "Extract"; kind: "primary"; onClicked: root.go() }
        }
    }
}
