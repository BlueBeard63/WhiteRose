// One card (CardsMode, and grouped in GroupedGrid): a picture or icon and
// what it is (kind, size, when); click, drag, drop on a folder, right-click.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F

Item {
    id: cell
    required property var modelData
    // Its place in the folder (what pane.clickAt takes).
    property int at: 0
    required property var pane
    // The grid it's in: k (size), cellWidth / cellHeight, current, focused, takeFocus(), now.
    required property var view
    readonly property bool on: !!cell.pane.selected[modelData.path]
    readonly property bool photo: String(modelData.mime).startsWith("image/") && modelData.size < 40e6
    width: cell.view.cellWidth; height: cell.view.cellHeight
    // Selected next to selected: one block (no gap, no corner, between them).
    readonly property var join: cell.pane.joined(cell.at, cell.view.cols)
    Rectangle {
        id: card
        // (Joined edges overlap a pixel: cells sit at fractional places.)
        anchors { fill: parent; leftMargin: cell.join.left ? -1 : 4; rightMargin: cell.join.right ? -1 : 4; topMargin: cell.join.up ? -1 : 4; bottomMargin: cell.join.down ? -1 : 4 }
        topLeftRadius: cell.join.left || cell.join.up ? 0 : 12
        topRightRadius: cell.join.right || cell.join.up ? 0 : 12
        bottomLeftRadius: cell.join.left || cell.join.down ? 0 : 12
        bottomRightRadius: cell.join.right || cell.join.down ? 0 : 12
        color: cell.on || drop.over ? Qt.tint(Theme.bg, Theme.tint(0.22)) : area.containsMouse ? Theme.surface2 : Theme.surface1
        border { width: drop.over || (cell.at === cell.view.current && cell.view.focused) ? 1 : 0; color: Theme.tint(0.6) }
        Rectangle {
            id: thumb
            x: cell.join.left ? 13 : 8; anchors.verticalCenter: parent.verticalCenter; anchors.verticalCenterOffset: (cell.join.up ? 2.5 : 0) - (cell.join.down ? 2.5 : 0)
            width: cell.height - 24; height: width
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
            CloudBadge { state: cell.pane.cloudOf(cell.modelData); size: 14 * cell.view.k; anchors { left: parent.left; bottom: parent.bottom } }
            CompareMark { look: cell.pane.compareOf(cell.modelData); size: 11; anchors { right: parent.right; top: parent.top; margins: 2 } }
        }
        Column {
            anchors { left: thumb.right; leftMargin: 10; right: parent.right; rightMargin: cell.join.right ? 15 : 10; verticalCenter: parent.verticalCenter; verticalCenterOffset: (cell.join.up ? 2.5 : 0) - (cell.join.down ? 2.5 : 0) }
            spacing: 2
            Loader {
                active: cell.pane.isNaming(cell.modelData)
                visible: active
                width: parent.width
                sourceComponent: NameEditor { pane: cell.pane; dir: !!cell.modelData.dir; fontSize: 12.5 * Math.sqrt(cell.view.k) }
            }
            Text {
                visible: !cell.pane.isNaming(cell.modelData)
                width: parent.width
                text: cell.modelData.name
                elide: Text.ElideMiddle
                color: cell.modelData.hidden ? Theme.muted : Theme.text
                font { family: Theme.font; pointSize: Theme.pt(12.5 * Math.sqrt(cell.view.k)); weight: Font.Medium }
            }
            Text {
                width: parent.width
                text: [cell.modelData.dir ? "Folder" : F.kind(cell.modelData.mime),
                       cell.modelData.dir ? (cell.pane.sizes[cell.modelData.path] ? F.size(cell.pane.sizes[cell.modelData.path].bytes) : "") : F.size(cell.modelData.size)]
                      .filter(x => x).join(" · ")
                elide: Text.ElideRight
                color: Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(11 * Math.sqrt(cell.view.k)) }
            }
            Row {
                spacing: 6
                TagDots { tags: cell.modelData.tags ?? []; anchors.verticalCenter: parent.verticalCenter }
                Text {
                    text: F.date(cell.modelData.modified, cell.view.now)
                    color: Theme.faint
                    font { family: Theme.font; pointSize: Theme.pt(10.5 * Math.sqrt(cell.view.k)) }
                }
            }
        }
    }
    DragSource { id: carry; paths: cell.pane.selection.map(e => e.path); icon: cell.modelData.icon ?? "" }
    FolderDrop { id: drop; anchors.fill: parent; enabled: cell.modelData.dir; dest: cell.modelData.path; onFilesDropped: (p, d) => cell.pane.dropped(p, d) }
    MouseArea {
        id: area
        anchors.fill: parent
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        onPositionChanged: m => { if (area.pressedButtons & Qt.LeftButton) carry.moved(m.x, m.y) }
        onPressed: m => {
            if (m.button === Qt.LeftButton) carry.press(m.x, m.y);
            if (m.button === Qt.LeftButton && !cell.on && !(m.modifiers & (Qt.ControlModifier | Qt.ShiftModifier)))
                cell.pane.clickAt(cell.at, false, false);
        }
        onClicked: m => {
            cell.view.takeFocus();
            if (m.button === Qt.RightButton) {
                if (!cell.on) cell.pane.clickAt(cell.at, false, false);
                const p = mapToItem(pane, m.x, m.y);
                cell.pane.menuAt(cell.modelData, p.x, p.y);
                return;
            }
            cell.pane.pick(cell.at, m.modifiers & Qt.ControlModifier, m.modifiers & Qt.ShiftModifier);
        }
        onDoubleClicked: cell.pane.activate(cell.modelData)
    }
}
