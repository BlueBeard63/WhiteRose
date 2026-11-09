// Gallery: one item at a time, as large as it fits. Left / Right (or the
// arrows at the sides on hover) move through; Enter or a double-click opens.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F

FocusScope {
    id: root
    required property var pane
    readonly property var item: root.pane.shownEntries[root.pane.current] ?? null
    focus: true
    function step(by) {
        const n = root.pane.shownEntries.length;
        if (n === 0) return;
        root.pane.clickAt(Math.max(0, Math.min(n - 1, root.pane.current + by)), false, false);
    }
    Keys.onLeftPressed: root.step(-1)
    Keys.onRightPressed: root.step(1)
    Keys.onReturnPressed: root.pane.activate(root.item)
    Keys.onEnterPressed: root.pane.activate(root.item)
    Component.onCompleted: if (root.pane.selection.length === 0 && root.pane.shownEntries.length > 0) root.pane.clickAt(root.pane.current, false, false)

    // ---- the one in front ----------------------------------------------------------
    Item {
        id: stage
        anchors { left: parent.left; right: parent.right; top: parent.top; bottom: caption.top; margins: 16; leftMargin: 64; rightMargin: 64 }
        Preview {
            anchors.fill: parent
            entry: root.item
            visible: !root.item?.look
        }
        FolderIcon {
            visible: !!root.item?.look
            anchors.centerIn: parent
            size: Math.min(parent.width, parent.height) * 0.45
            look: root.item?.look ?? ({})
        }
        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            // (Video and audio have their own controls: clicks go through.)
            enabled: !["video", "audio"].includes(String(root.item?.mime ?? "").split("/")[0])
            onDoubleClicked: root.pane.activate(root.item)
            onClicked: m => {
                root.forceActiveFocus();
                if (m.button === Qt.RightButton && root.item) { const p = mapToItem(root.pane, m.x, m.y); root.pane.menuAt(root.item, p.x, p.y) }
            }
        }
    }
    Column {
        id: caption
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom; bottomMargin: 16; leftMargin: 16; rightMargin: 16 }
        spacing: 2
        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: root.item?.name ?? ""
            elide: Text.ElideMiddle
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.Medium }
        }
        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: root.item ? [root.item.dir ? "Folder" : F.kind(root.item.mime), root.item.dir ? "" : F.size(root.item.size),
                               F.date(root.item.modified, Date.now()),
                               (root.pane.current + 1) + " of " + root.pane.shownEntries.length].filter(x => x).join(" · ") : ""
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(11.5) }
        }
    }

    // ---- previous / next: arrows at the sides while the pointer's over it ----------------
    HoverHandler { id: hovering }
    component Arrow: Rectangle {
        id: arrow
        property int by: 1
        visible: hovering.hovered && (by < 0 ? root.pane.current > 0 : root.pane.current < root.pane.shownEntries.length - 1)
        anchors.verticalCenter: stage.verticalCenter
        width: 40; height: 40; radius: 20
        color: arrowArea.containsMouse ? Theme.surface2 : Qt.rgba(Theme.surface1.r, Theme.surface1.g, Theme.surface1.b, 0.85)
        Icon { anchors.centerIn: parent; name: arrow.by < 0 ? "chevL" : "chevR"; size: 18; color: Theme.text }
        MouseArea { id: arrowArea; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: { root.forceActiveFocus(); root.step(arrow.by) } }
    }
    Arrow { by: -1; anchors.left: parent.left; anchors.leftMargin: 14 }
    Arrow { by: 1; anchors.right: parent.right; anchors.rightMargin: 14 }
}
