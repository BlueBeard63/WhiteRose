// Cards: bigger tiles with a picture or icon and what each is (kind,
// size, when). Clicks, keys, dragging and dropping as in the grid.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F

GridView {
    id: grid
    required property var pane
    readonly property real k: Session.zoom
    readonly property real inner: width - leftMargin - rightMargin
    readonly property real now: Date.now()
    model: pane.shownEntries
    currentIndex: pane.current
    cellWidth: inner / Math.max(1, Math.floor(inner / (250 * k)))
    cellHeight: 80 * k
    clip: true
    focus: true
    boundsBehavior: Flickable.StopAtBounds
    leftMargin: 10; rightMargin: 10; topMargin: 10; bottomMargin: 10
    ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
    onCurrentIndexChanged: if (currentIndex !== pane.current) pane.current = currentIndex
    highlightFollowsCurrentItem: false
    keyNavigationEnabled: true
    Keys.onReturnPressed: pane.activate(pane.shownEntries[pane.current])
    Keys.onEnterPressed: pane.activate(pane.shownEntries[pane.current])
    Keys.onPressed: e => {
        if ([Qt.Key_Left, Qt.Key_Right, Qt.Key_Up, Qt.Key_Down].includes(e.key))
            Qt.callLater(() => pane.clickAt(grid.currentIndex, false, (e.modifiers & Qt.ShiftModifier) !== 0));
    }
    MouseArea {
        anchors.fill: parent
        z: -1
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onClicked: m => {
            pane.selected = {};
            grid.forceActiveFocus();
            if (m.button === Qt.RightButton) { const p = mapToItem(pane, m.x, m.y); pane.menuAt(null, p.x, p.y) }
        }
    }

    delegate: Item {
        id: cell
        required property var modelData
        required property int index
        readonly property bool on: !!pane.selected[modelData.path]
        readonly property bool photo: String(modelData.mime).startsWith("image/") && modelData.size < 40e6
        width: grid.cellWidth; height: grid.cellHeight
        Rectangle {
            id: card
            anchors { fill: parent; margins: 4 }
            radius: 12
            color: cell.on || drop.over ? Theme.tint(0.22) : area.containsMouse ? Theme.surface2 : Theme.surface1
            border { width: drop.over || (cell.index === grid.currentIndex && grid.activeFocus) ? 1 : 0; color: Theme.tint(0.6) }
            Rectangle {
                id: thumb
                x: 8; anchors.verticalCenter: parent.verticalCenter
                width: card.height - 16; height: width
                radius: 8
                color: cell.photo ? Theme.bg : "transparent"
                clip: true
                Image {
                    anchors { fill: parent; margins: cell.photo ? 0 : 4 }
                    visible: !cell.modelData.look
                    source: cell.photo ? "file://" + cell.modelData.path : cell.modelData.icon ? "file://" + cell.modelData.icon : ""
                    sourceSize: Qt.size(width * 2, height * 2)
                    fillMode: cell.photo ? Image.PreserveAspectCrop : Image.PreserveAspectFit
                    asynchronous: true
                    autoTransform: true
                    opacity: cell.modelData.hidden ? 0.55 : 1
                }
                FolderIcon { visible: !!cell.modelData.look; anchors.centerIn: parent; size: parent.width * 0.85; look: cell.modelData.look ?? ({}) }
            }
            Column {
                anchors { left: thumb.right; leftMargin: 10; right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
                spacing: 2
                Text {
                    width: parent.width
                    text: cell.modelData.name
                    elide: Text.ElideMiddle
                    color: cell.modelData.hidden ? Theme.muted : Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(12.5 * Math.sqrt(grid.k)); weight: Font.Medium }
                }
                Text {
                    width: parent.width
                    text: [cell.modelData.dir ? "Folder" : F.kind(cell.modelData.mime),
                           cell.modelData.dir ? (pane.sizes[cell.modelData.path] ? F.size(pane.sizes[cell.modelData.path].bytes) : "") : F.size(cell.modelData.size)]
                          .filter(x => x).join(" · ")
                    elide: Text.ElideRight
                    color: Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(11 * Math.sqrt(grid.k)) }
                }
                Text {
                    width: parent.width
                    text: F.date(cell.modelData.modified, grid.now)
                    color: Theme.faint
                    font { family: Theme.font; pointSize: Theme.pt(10.5 * Math.sqrt(grid.k)) }
                }
            }
        }
        DragSource { id: carry; paths: pane.selection.map(e => e.path); icon: cell.modelData.icon ?? "" }
        FolderDrop { id: drop; anchors.fill: parent; enabled: cell.modelData.dir; dest: cell.modelData.path; onFilesDropped: (p, d) => pane.dropped(p, d) }
        MouseArea {
            id: area
            anchors.fill: parent
            hoverEnabled: true
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            onPositionChanged: m => { if (area.pressedButtons & Qt.LeftButton) carry.moved(m.x, m.y) }
            onPressed: m => {
                if (m.button === Qt.LeftButton) carry.press(m.x, m.y);
                if (m.button === Qt.LeftButton && !cell.on && !(m.modifiers & (Qt.ControlModifier | Qt.ShiftModifier)))
                    pane.clickAt(cell.index, false, false);
            }
            onClicked: m => {
                grid.forceActiveFocus();
                if (m.button === Qt.RightButton) {
                    if (!cell.on) pane.clickAt(cell.index, false, false);
                    const p = mapToItem(pane, m.x, m.y);
                    pane.menuAt(cell.modelData, p.x, p.y);
                    return;
                }
                pane.clickAt(cell.index, m.modifiers & Qt.ControlModifier, m.modifiers & Qt.ShiftModifier);
            }
            onDoubleClicked: pane.activate(cell.modelData)
        }
    }
}
