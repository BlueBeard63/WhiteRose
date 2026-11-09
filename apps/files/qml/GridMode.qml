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
        onClicked: { pane.selected = {}; grid.forceActiveFocus() }
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
            color: cell.on ? Theme.tint(0.22) : area.containsMouse ? Theme.surface1 : "transparent"
            border { width: cell.index === grid.currentIndex && grid.activeFocus ? 1 : 0; color: Theme.tint(0.6) }
        }
        Image {
            id: icon
            anchors { horizontalCenter: parent.horizontalCenter; top: parent.top; topMargin: 12 }
            width: 48; height: 48
            sourceSize: Qt.size(96, 96)
            source: cell.modelData.icon ? "file://" + cell.modelData.icon : ""
            asynchronous: true
            opacity: cell.modelData.hidden ? 0.55 : 1
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
            wrapMode: Text.WrapAnywhere
            maximumLineCount: 2
            elide: Text.ElideMiddle
            color: cell.modelData.hidden ? Theme.muted : Theme.text
            font { family: Theme.font; pointSize: Theme.pt(12) }
        }
        MouseArea {
            id: area
            anchors.fill: parent
            hoverEnabled: true
            onClicked: m => { pane.clickAt(cell.index, m.modifiers & Qt.ControlModifier, m.modifiers & Qt.ShiftModifier); grid.forceActiveFocus() }
            onDoubleClicked: pane.activate(cell.modelData)
        }
    }
}
