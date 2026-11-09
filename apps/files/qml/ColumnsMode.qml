// Columns: every folder on the way to this one, side by side, each
// showing which child is open. Click a folder in any column to go there;
// the last column is this folder (selection, opening files).
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files

FocusScope {
    id: root
    required property var pane
    readonly property var chain: JSON.parse(Fs.crumbs(pane.path))
    // Each earlier column's listing (the last one is the pane's own).
    readonly property var lists: {
        void pane.stamp; void pane.hidden;
        return chain.slice(0, -1).map(c => {
            const r = JSON.parse(Fs.list(c.path, pane.hidden, "name", false, 20));
            return r.ok ? r.entries : [];
        });
    }

    ListView {
        id: columns
        anchors.fill: parent
        orientation: ListView.Horizontal
        model: root.chain.length
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.horizontal: ScrollBar { policy: ScrollBar.AsNeeded }
        onCountChanged: Qt.callLater(() => columns.positionViewAtEnd())

        delegate: Rectangle {
            id: col
            required property int index
            readonly property bool last: index === root.chain.length - 1
            readonly property var entries: last ? root.pane.shownEntries : (root.lists[index] ?? [])
            // The child of this column that's on the way (earlier columns).
            readonly property string openChild: last ? "" : (root.chain[index + 1]?.path ?? "")
            width: 240; height: columns.height
            color: Theme.bg
            Rectangle { anchors.right: parent.right; width: 1; height: parent.height; color: Theme.surface1 }

            ListView {
                id: list
                anchors { fill: parent; rightMargin: 1 }
                model: col.entries
                clip: true
                focus: col.last
                boundsBehavior: Flickable.StopAtBounds
                topMargin: 6; bottomMargin: 6
                ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                currentIndex: col.last ? root.pane.current : col.entries.findIndex(e => e.path === col.openChild)
                Component.onCompleted: if (!col.last && currentIndex >= 0) positionViewAtIndex(currentIndex, ListView.Center)
                keyNavigationEnabled: col.last
                onCurrentIndexChanged: if (col.last && currentIndex !== root.pane.current && currentIndex >= 0) root.pane.current = currentIndex
                Keys.onReturnPressed: root.pane.activate(root.pane.shownEntries[root.pane.current])
                Keys.onRightPressed: { const e = root.pane.shownEntries[root.pane.current]; if (e?.dir) root.pane.go(e.path) }
                Keys.onLeftPressed: root.pane.up()

                delegate: Rectangle {
                    id: row
                    required property var modelData
                    required property int index
                    readonly property bool on: col.last ? !!root.pane.selected[modelData.path] : modelData.path === col.openChild
                    x: 6; width: list.width - 12; height: 30
                    radius: 8
                    color: on || drop.over ? Theme.tint(col.last || drop.over ? 0.22 : 0.12) : rowArea.containsMouse ? Theme.surface1 : "transparent"
                    border { width: drop.over ? 1 : 0; color: Theme.tint(0.6) }
                    Image {
                        visible: !row.modelData.look
                        id: ic
                        x: 8; anchors.verticalCenter: parent.verticalCenter
                        width: 18; height: 18
                        sourceSize: Qt.size(36, 36)
                        source: row.modelData.icon ? "file://" + row.modelData.icon : ""
                        asynchronous: true
                        opacity: row.modelData.hidden ? 0.55 : 1
                    }
                    // A folder with its own colour or icon.
                    FolderIcon {
                        visible: !!row.modelData.look
                        anchors.fill: ic
                        size: ic.width
                        look: row.modelData.look ?? ({})
                    }
                    Text {
                        anchors { left: ic.right; leftMargin: 8; right: chev.left; rightMargin: 4; verticalCenter: parent.verticalCenter }
                        text: row.modelData.name
                        elide: Text.ElideMiddle
                        color: row.modelData.hidden ? Theme.muted : Theme.text
                        font { family: Theme.font; pointSize: Theme.pt(12.5) }
                    }
                    Icon { id: chev; visible: row.modelData.dir; anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter } name: "chevR"; size: 11; color: Theme.faint }
                    // In this folder's column the selection is dragged; in an
                    // earlier one, just the item.
                    DragSource {
                        id: carry
                        paths: col.last ? root.pane.selection.map(e => e.path) : [row.modelData.path]
                        icon: row.modelData.icon ?? ""
                    }
                    FolderDrop { id: drop; anchors.fill: parent; enabled: row.modelData.dir; dest: row.modelData.path; onFilesDropped: (p, d) => root.pane.dropped(p, d) }
                    MouseArea {
                        id: rowArea
                        anchors.fill: parent
                        hoverEnabled: true
                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                        onPositionChanged: m => { if (rowArea.pressedButtons & Qt.LeftButton) carry.moved(m.x, m.y) }
                        onPressed: m => {
                            if (m.button === Qt.LeftButton) carry.press(m.x, m.y);
                            if (col.last && m.button === Qt.LeftButton && !row.on && !(m.modifiers & (Qt.ControlModifier | Qt.ShiftModifier))) {
                                const i = root.pane.shownEntries.findIndex(e => e.path === row.modelData.path);
                                if (i >= 0) root.pane.clickAt(i, false, false);
                            }
                        }
                        onClicked: m => {
                            if (m.button === Qt.RightButton) {
                                // An earlier column's item: its folder becomes the open one first.
                                const p = mapToItem(root.pane, m.x, m.y), path = row.modelData.path, pane = root.pane;
                                if (!col.last) pane.go(root.chain[col.index].path);
                                const i = pane.shownEntries.findIndex(e => e.path === path);
                                if (i >= 0 && !pane.selected[path]) pane.clickAt(i, false, false);
                                pane.menuAt(i >= 0 ? pane.shownEntries[i] : null, p.x, p.y);
                                return;
                            }
                            if (row.modelData.dir) root.pane.go(row.modelData.path);
                            else if (col.last) root.pane.clickAt(row.index, m.modifiers & Qt.ControlModifier, m.modifiers & Qt.ShiftModifier);
                            else root.pane.go(root.chain[col.index].path);
                        }
                        onDoubleClicked: if (!row.modelData.dir) root.pane.activate(row.modelData)
                    }
                }
            }
        }
    }
}
