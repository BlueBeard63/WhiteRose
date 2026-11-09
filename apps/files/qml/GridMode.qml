// Icons in a grid. Click selects (Ctrl toggles, Shift a range), double-click
// or Enter opens, arrows move, Backspace goes up.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files

GridView {
    id: grid
    required property var pane
    model: pane.shownEntries
    currentIndex: pane.current
    cellWidth: Math.max(108, width / Math.max(1, Math.floor(width / 116)))
    cellHeight: 112
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
    // Moving with the keys selects (Shift extends).
    Keys.onPressed: e => {
        if ([Qt.Key_Left, Qt.Key_Right, Qt.Key_Up, Qt.Key_Down].includes(e.key)) {
            Qt.callLater(() => pane.clickAt(grid.currentIndex, false, (e.modifiers & Qt.ShiftModifier) !== 0));
        }
    }

    // A click on the empty space clears the selection.
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
        width: grid.cellWidth; height: grid.cellHeight
        Rectangle {
            anchors { fill: parent; margins: 4 }
            radius: 12
            color: cell.on || drop.over ? Theme.tint(0.22) : area.containsMouse ? Theme.surface1 : "transparent"
            border { width: drop.over || (cell.index === grid.currentIndex && grid.activeFocus) ? 1 : 0; color: Theme.tint(0.6) }
        }
        // Pictures as themselves (decoded at icon size; up to 40 MB), the
        // rest, or a picture that won't load, as their type's icon.
        property bool thumbFailed: false
        readonly property bool thumb: !cell.thumbFailed && String(cell.modelData.mime).startsWith("image/") && cell.modelData.size < 40e6
        Image {
            visible: !cell.modelData.look
            id: icon
            anchors { horizontalCenter: parent.horizontalCenter; top: parent.top; topMargin: cell.thumb ? 0 : 12 }
            width: cell.thumb ? 60 : 48; height: width
            sourceSize: Qt.size(width * 2, height * 2)
            fillMode: Image.PreserveAspectFit
            source: cell.thumb ? "file://" + cell.modelData.path : cell.modelData.icon ? "file://" + cell.modelData.icon : ""
            asynchronous: true
            autoTransform: true
            opacity: cell.modelData.hidden ? 0.55 : 1
            onStatusChanged: if (icon.status === Image.Error && cell.thumb) cell.thumbFailed = true
        }
        // A folder with its own colour or icon.
        FolderIcon {
            visible: !!cell.modelData.look
            anchors.fill: icon
            size: icon.width
            look: cell.modelData.look ?? ({})
        }
        // A link: a small arrow on the icon.
        Rectangle {
            visible: cell.modelData.link
            anchors { right: icon.right; bottom: icon.bottom }
            width: 16; height: 16; radius: 8
            color: Theme.surface2
            Icon { anchors.centerIn: parent; name: "link"; size: 10; color: Theme.text }
        }
        Text {
            anchors { top: icon.bottom; topMargin: 6; left: parent.left; right: parent.right; leftMargin: 8; rightMargin: 8 }
            horizontalAlignment: Text.AlignHCenter
            text: cell.modelData.name
            wrapMode: Text.WrapAtWordBoundaryOrAnywhere
            maximumLineCount: 2
            elide: Text.ElideMiddle
            color: cell.modelData.hidden ? Theme.muted : Theme.text
            font { family: Theme.font; pointSize: Theme.pt(12) }
        }
        // Dragged: the selection goes (pressing an unselected item selects it first).
        DragSource { id: carry; paths: pane.selection.map(e => e.path); icon: cell.modelData.icon ?? ""; }
        // A folder takes drops.
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
                    // Right-click on something not selected: just that.
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
