// A closer look (Space): the file filling the window. Arrows go through
// the folder, Enter opens it, Space or Esc closes.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F

Popup {
    id: root
    property var entry: null
    signal step(int by)
    signal openIt(var entry)

    parent: Overlay.overlay
    anchors.centerIn: parent
    width: parent ? parent.width - 80 : 800
    height: parent ? parent.height - 80 : 600
    modal: true
    focus: true
    padding: 0
    background: Rectangle { color: Theme.bg; radius: 16; border { width: 1; color: Theme.surface2 } }
    Overlay.modal: Rectangle { color: Qt.rgba(0, 0, 0, 0.55) }

    contentItem: FocusScope {
        focus: true
        Keys.onPressed: e => {
            if (e.key === Qt.Key_Space || e.key === Qt.Key_Escape) { root.close(); e.accepted = true }
            else if (e.key === Qt.Key_Left || e.key === Qt.Key_Up) { root.step(-1); e.accepted = true }
            else if (e.key === Qt.Key_Right || e.key === Qt.Key_Down) { root.step(1); e.accepted = true }
            else if (e.key === Qt.Key_Return || e.key === Qt.Key_Enter) {
                // Once the key's handled (closing during it can crash Qt).
                const it = root.entry;
                Qt.callLater(() => { root.close(); root.openIt(it) });
                e.accepted = true;
            }
        }

        Item {
            id: head
            anchors { left: parent.left; right: parent.right; top: parent.top }
            height: 54
            Column {
                anchors { left: parent.left; leftMargin: 20; right: buttons.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
                spacing: 2
                Text {
                    width: parent.width
                    text: root.entry?.name ?? ""
                    elide: Text.ElideMiddle
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(13.5); weight: Font.DemiBold }
                }
                Text {
                    width: parent.width
                    text: root.entry ? [root.entry.dir ? "Folder" : F.kind(root.entry.mime), look.facts,
                                        root.entry.dir ? "" : F.size(root.entry.size)].filter(x => x).join(" · ") : ""
                    elide: Text.ElideRight
                    color: Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(11.5) }
                }
            }
            Row {
                id: buttons
                anchors { right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
                spacing: 6
                Button { text: "Open"; kind: "primary"; onClicked: { const it = root.entry; root.close(); root.openIt(it) } }
                IconButton { width: 30; height: 30; icon: "x"; iconSize: 13; fill: "transparent"; onClicked: root.close() }
            }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.surface1 }
        }
        Preview {
            id: look
            anchors { left: parent.left; right: parent.right; top: head.bottom; bottom: parent.bottom; margins: 16 }
            entry: root.visible ? root.entry : null
            big: true
        }
    }
}
