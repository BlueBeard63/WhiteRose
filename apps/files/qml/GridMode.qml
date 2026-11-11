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
    // Item size (Ctrl+= / Ctrl+-).
    readonly property real k: Session.zoom
    readonly property real inner: width - leftMargin - rightMargin
    cellWidth: inner / Math.max(1, Math.floor(inner / (116 * k)))
    cellHeight: 112 * k
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
            pane.commitNaming();
            pane.selected = {};
            grid.forceActiveFocus();
            if (m.button === Qt.RightButton) { const p = mapToItem(pane, m.x, m.y); pane.menuAt(null, p.x, p.y) }
        }
    }

    // Grid-wide what each cell needs.
    readonly property int current: currentIndex
    readonly property bool focused: activeFocus
    function takeFocus() { grid.forceActiveFocus() }

    delegate: GridCell {
        required property int index
        at: index
        pane: grid.pane
        view: grid
    }
}
