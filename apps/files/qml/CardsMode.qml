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

    delegate: CardCell {
        required property int index
        at: index
        pane: grid.pane
        view: grid
    }
}
