// One item in the grid (GridMode, and grouped in GroupedGrid): its icon or
// picture, its name; click, drag, drop on a folder, right-click.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as Fmt

Item {
    id: cell
    required property var modelData
    // Its place in the folder (what pane.clickAt takes).
    property int at: 0
    required property var pane
    // The grid it's in: k (size), cellWidth / cellHeight, current, focused, takeFocus(), now.
    required property var view
    readonly property bool on: !!cell.pane.selected[modelData.path]
    // (For screen readers and the UI tests: the file, whether it's
    // selected, and, while renaming, its new name.)
    Accessible.role: Accessible.ListItem
    Accessible.name: cell.modelData.name ?? ""
    Accessible.description: ""
    Accessible.selectable: true
    Accessible.selected: cell.on

    width: cell.view.cellWidth; height: cell.view.cellHeight
    // Selected next to selected: one block (no gap, no corner, between them).
    readonly property var join: cell.pane.joined(cell.at, cell.view.cols)
    Rectangle {
        // (Joined edges overlap a pixel: cells sit at fractional places.)
        anchors { fill: parent; leftMargin: cell.join.left ? -1 : 4; rightMargin: cell.join.right ? -1 : 4; topMargin: cell.join.up ? -1 : 4; bottomMargin: cell.join.down ? -1 : 4 }
        topLeftRadius: cell.join.left || cell.join.up ? 0 : 12
        topRightRadius: cell.join.right || cell.join.up ? 0 : 12
        bottomLeftRadius: cell.join.left || cell.join.down ? 0 : 12
        bottomRightRadius: cell.join.right || cell.join.down ? 0 : 12
        color: cell.on || drop.over ? Qt.tint(Theme.bg, Theme.tint(0.22)) : area.containsMouse ? Theme.surface1 : "transparent"
        border { width: drop.over || (cell.at === cell.view.current && cell.view.focused) ? 1 : 0; color: Theme.tint(0.6) }
    }
    // Pictures as themselves (decoded at icon size; up to 40 MB), the
    // rest, or a picture that won't load, as their type's icon.
    property bool thumbFailed: false
    readonly property bool thumb: !cell.thumbFailed && String(cell.modelData.mime).startsWith("image/") && cell.modelData.size < 40e6
    Image {
        visible: !cell.modelData.look
        id: icon
        anchors { horizontalCenter: parent.horizontalCenter; top: parent.top; topMargin: (cell.thumb ? 0 : 12) * cell.view.k }
        width: (cell.thumb ? 60 : 48) * cell.view.k; height: width
        sourceSize: Qt.size(width * 2, height * 2)
        fillMode: Image.PreserveAspectFit
        source: cell.thumb ? Fmt.fileUrl(cell.modelData.path) : cell.modelData.icon ? Fmt.fileUrl(cell.modelData.icon) : ""
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
        visible: !!cell.modelData?.link
        anchors { right: icon.right; bottom: icon.bottom }
        width: 16; height: 16; radius: 8
        color: Theme.surface2
        Icon { anchors.centerIn: parent; name: "link"; size: 10; color: Theme.text }
    }
    // Comparing: what differs, at the top corner.
    CompareMark { look: cell.pane.compareOf(cell.modelData); size: 11; anchors { right: icon.right; rightMargin: -4; top: icon.top; topMargin: -2 } }
    // Sync client: its state at the other corner.
    CloudBadge {
        state: cell.pane.cloudOf(cell.modelData)
        size: 14 * cell.view.k
        anchors { left: icon.right; leftMargin: -8 * cell.view.k; bottom: icon.bottom }
    }
    // Its tags: dots at the icon's corner.
    TagDots {
        tags: cell.modelData.tags ?? []
        anchors { left: icon.right; leftMargin: -4; top: icon.top }
        size: 8 * cell.view.k
    }
    // Being named (new, or renamed): typed right here.
    Loader {
        active: cell.pane.isNaming(cell.modelData)
        anchors { top: icon.bottom; topMargin: 4; left: parent.left; right: parent.right; leftMargin: 6; rightMargin: 6 }
        sourceComponent: NameEditor { pane: cell.pane; dir: !!cell.modelData.dir; align: TextInput.AlignHCenter; fontSize: 12 * Math.sqrt(cell.view.k) }
    }
    Text {
        visible: !cell.pane.isNaming(cell.modelData)
        anchors { top: icon.bottom; topMargin: 6; left: parent.left; right: parent.right; leftMargin: 8; rightMargin: 8 }
        horizontalAlignment: Text.AlignHCenter
        text: cell.modelData.name
        wrapMode: Text.WrapAtWordBoundaryOrAnywhere
        maximumLineCount: 2
        elide: Text.ElideMiddle
        color: cell.modelData.hidden ? Theme.muted : Theme.text
        font { family: Theme.font; pointSize: Theme.pt(12 * Math.sqrt(cell.view.k)) }
    }
    // Dragged: the selection goes (pressing an unselected item selects it first).
    DragSource { id: carry; paths: cell.pane.selection.map(e => e.path); icon: cell.modelData?.icon ?? ""; }
    // A folder takes drops.
    FolderDrop { id: drop; anchors.fill: parent; enabled: !!cell.modelData?.dir; dest: cell.modelData?.path ?? ""; onFilesDropped: (p, d) => cell.pane.dropped(p, d) }
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
                // Right-click on something not selected: just that.
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
