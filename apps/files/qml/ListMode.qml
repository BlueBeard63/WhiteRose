// Details: name, size, modified and kind in columns. Click a column's
// heading to sort by it (again to reverse).
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F

FocusScope {
    id: root
    required property var pane
    readonly property real sizeW: 90
    readonly property real dateW: 150
    // In the Trash the last column is where things came from.
    readonly property real kindW: root.pane.inTrash ? 220 : 110
    readonly property real now: Date.now()

    // ---- headings -------------------------------------------------------------
    Rectangle {
        id: head
        width: parent.width; height: 30
        color: Theme.bg
        Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.surface1 }
        Row {
            anchors { fill: parent; leftMargin: 16; rightMargin: 16 }
            Repeater {
                model: [
                    { key: "name", label: "Name", w: head.width - 32 - root.sizeW - root.dateW - root.kindW },
                    { key: "size", label: "Size", w: root.sizeW },
                    { key: "modified", label: root.pane.inTrash ? "Deleted" : "Modified", w: root.dateW },
                    { key: root.pane.inTrash ? "" : "kind", label: root.pane.inTrash ? "From" : "Kind", w: root.kindW },
                ]
                delegate: Item {
                    required property var modelData
                    width: modelData.w; height: head.height
                    Row {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 4
                        Text {
                            text: modelData.label
                            color: root.pane.sortBy === modelData.key ? Theme.text : Theme.muted
                            font { family: Theme.font; pointSize: Theme.pt(11.5); weight: Font.Medium }
                        }
                        Icon {
                            visible: root.pane.sortBy === modelData.key
                            anchors.verticalCenter: parent.verticalCenter
                            name: root.pane.descending ? "chevDown" : "chevUp"; size: 11; color: Theme.muted
                        }
                    }
                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            if (root.pane.sortBy === modelData.key) root.pane.setView({ descending: !root.pane.descending });
                            else if (modelData.key !== "") root.pane.setView({ sortBy: modelData.key, descending: modelData.key !== "name" && modelData.key !== "kind" });
                        }
                    }
                }
            }
        }
    }

    // ---- rows ----------------------------------------------------------------------
    ListView {
        id: list
        anchors { left: parent.left; right: parent.right; top: head.bottom; bottom: parent.bottom }
        model: root.pane.shownEntries
        currentIndex: root.pane.current
        clip: true
        focus: true
        boundsBehavior: Flickable.StopAtBounds
        topMargin: 4; bottomMargin: 8
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
        highlightFollowsCurrentItem: false
        keyNavigationEnabled: true
        onCurrentIndexChanged: if (currentIndex !== root.pane.current) root.pane.current = currentIndex
        Keys.onReturnPressed: root.pane.activate(root.pane.shownEntries[root.pane.current])
        Keys.onEnterPressed: root.pane.activate(root.pane.shownEntries[root.pane.current])
        Keys.onPressed: e => {
            if (e.key === Qt.Key_Up || e.key === Qt.Key_Down)
                Qt.callLater(() => root.pane.clickAt(list.currentIndex, false, (e.modifiers & Qt.ShiftModifier) !== 0));
        }
        MouseArea {
            anchors.fill: parent
            z: -1
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            onClicked: m => {
                root.pane.selected = {};
                list.forceActiveFocus();
                if (m.button === Qt.RightButton) { const p = mapToItem(root.pane, m.x, m.y); root.pane.menuAt(null, p.x, p.y) }
            }
        }

        delegate: Rectangle {
            id: row
            required property var modelData
            required property int index
            readonly property bool on: !!root.pane.selected[modelData.path]
            x: 8; width: list.width - 16; height: 32
            radius: 8
            color: on || drop.over ? Theme.tint(0.22) : rowArea.containsMouse ? Theme.surface1 : "transparent"
            border { width: drop.over || (index === list.currentIndex && list.activeFocus) ? 1 : 0; color: Theme.tint(0.6) }
            Row {
                anchors { fill: parent; leftMargin: 8; rightMargin: 8 }
                Item {
                    width: head.width - 32 - root.sizeW - root.dateW - root.kindW; height: parent.height
                    Image {
                        id: ic
                        anchors.verticalCenter: parent.verticalCenter
                        width: 20; height: 20
                        sourceSize: Qt.size(40, 40)
                        source: row.modelData.icon ? "file://" + row.modelData.icon : ""
                        asynchronous: true
                        opacity: row.modelData.hidden ? 0.55 : 1
                    }
                    Text {
                        anchors { left: ic.right; leftMargin: 10; right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
                        text: row.modelData.name + (row.modelData.link ? "  →" : "")
                        elide: Text.ElideMiddle
                        color: row.modelData.hidden ? Theme.muted : Theme.text
                        font { family: Theme.font; pointSize: Theme.pt(12.5) }
                    }
                }
                Text { width: root.sizeW; anchors.verticalCenter: parent.verticalCenter; text: row.modelData.dir ? "—" : F.size(row.modelData.size); color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(12) } }
                Text { width: root.dateW; anchors.verticalCenter: parent.verticalCenter; text: F.date(row.modelData.modified, root.now); color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(12) } }
                Text { width: root.kindW; anchors.verticalCenter: parent.verticalCenter; text: row.modelData.origin !== undefined ? row.modelData.origin.replace(Fs.home(), "~") : F.kind(row.modelData.mime); elide: root.pane.inTrash ? Text.ElideMiddle : Text.ElideRight; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(12) } }
            }
            DragSource { id: carry; paths: root.pane.selection.map(e => e.path); icon: row.modelData.icon ?? ""; }
            FolderDrop { id: drop; anchors.fill: parent; enabled: row.modelData.dir; dest: row.modelData.path; onFilesDropped: (p, d) => root.pane.dropped(p, d) }
            MouseArea {
                id: rowArea
                anchors.fill: parent
                hoverEnabled: true
                acceptedButtons: Qt.LeftButton | Qt.RightButton
                onPositionChanged: m => { if (rowArea.pressedButtons & Qt.LeftButton) carry.moved(m.x, m.y) }
                onPressed: m => {
                    if (m.button === Qt.LeftButton) carry.press(m.x, m.y);
                    if (m.button === Qt.LeftButton && !row.on && !(m.modifiers & (Qt.ControlModifier | Qt.ShiftModifier)))
                        root.pane.clickAt(row.index, false, false);
                }
                onClicked: m => {
                    list.forceActiveFocus();
                    if (m.button === Qt.RightButton) {
                        if (!row.on) root.pane.clickAt(row.index, false, false);
                        const p = mapToItem(root.pane, m.x, m.y);
                        root.pane.menuAt(row.modelData, p.x, p.y);
                        return;
                    }
                    root.pane.clickAt(row.index, m.modifiers & Qt.ControlModifier, m.modifiers & Qt.ShiftModifier);
                }
                onDoubleClicked: root.pane.activate(row.modelData)
            }
        }
    }
}
