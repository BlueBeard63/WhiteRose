// An archive wants a password (extracting said so): asked, then tried again.
// Kept in memory for this run only. Asked in a strip above the files.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files

Strip {
    id: root
    icon: "lock"
    focusOn: pw
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

    Text {
        anchors.verticalCenter: parent.verticalCenter
        width: Math.min(implicitWidth, 300)
        text: "“" + root.archive.substring(root.archive.lastIndexOf("/") + 1) + "” needs a password"
        elide: Text.ElideMiddle
        color: Theme.text
        font { family: Theme.font; pointSize: Theme.pt(12); weight: Font.Medium }
    }
    Field { id: pw; anchors.verticalCenter: parent.verticalCenter; width: 200; height: 30; secret: true; placeholder: "Password"; error: root.wrong; onAccepted: Qt.callLater(root.go) }
    Text {
        anchors.verticalCenter: parent.verticalCenter
        visible: root.wrong
        text: "Wrong password. Try again."
        color: Theme.rose
        font { family: Theme.font; pointSize: Theme.pt(10.5) }
    }
    buttons: [
        Button { text: "Cancel"; kind: "ghost"; onClicked: root.close() },
        Button { text: "Extract"; kind: "primary"; onClicked: root.go() }
    ]
}
