// The grid or cards grouped (Group by): a heading per group (click to fold
// it away), then its items in rows. Arrow keys move through the rows;
// otherwise each item behaves as in the plain grid.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Arrange.js" as Arrange

ListView {
    id: grid
    required property var pane
    // Cards rather than icons.
    property bool cards: false
    // What each cell needs (GridCell / CardCell).
    readonly property real k: Session.zoom
    readonly property real inner: width - leftMargin - rightMargin
    readonly property int cols: Math.max(1, Math.floor(inner / ((cards ? 250 : 116) * k)))
    readonly property real cellWidth: inner / cols
    readonly property real cellHeight: (cards ? 80 : 112) * k
    readonly property int current: pane.current
    readonly property bool focused: activeFocus
    readonly property real now: Date.now()
    function takeFocus() { grid.forceActiveFocus() }

    readonly property var rowList: Arrange.rows(pane.shownEntries, cols, pane.folded)
    model: grid.rowList
    clip: true
    focus: true
    boundsBehavior: Flickable.StopAtBounds
    leftMargin: 10; rightMargin: 10; topMargin: 4; bottomMargin: 10
    ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

    function move(key, shift) {
        const to = Arrange.step(grid.rowList, pane.current, key);
        if (to < 0) return;
        pane.clickAt(to, false, shift);
        const r = grid.rowList.findIndex(x => x.items && x.items.some(it => it.i === to));
        if (r >= 0) grid.positionViewAtIndex(r, ListView.Contain);
    }
    Keys.onPressed: e => {
        const key = { [Qt.Key_Left]: "left", [Qt.Key_Right]: "right", [Qt.Key_Up]: "up", [Qt.Key_Down]: "down" }[e.key];
        if (!key) return;
        grid.move(key, (e.modifiers & Qt.ShiftModifier) !== 0);
        e.accepted = true;
    }
    Keys.onReturnPressed: pane.activate(pane.shownEntries[pane.current])
    Keys.onEnterPressed: pane.activate(pane.shownEntries[pane.current])

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
        id: row
        required property var modelData
        width: grid.inner
        height: row.modelData.header !== undefined ? 40 : grid.cellHeight

        GroupHeading {
            visible: row.modelData.header !== undefined
            anchors { fill: parent; topMargin: 8 }
            label: row.modelData.header ?? ""
            count: row.modelData.count ?? 0
            folded: !!row.modelData.folded
            onToggled: row.modelData.header !== undefined && pane.fold(row.modelData.header)
        }
        Row {
            visible: !!row.modelData.items
            Repeater {
                model: row.modelData.items ?? []
                delegate: Loader {
                    id: slot
                    required property var modelData
                    sourceComponent: grid.cards ? cardCell : gridCell
                    Component { id: gridCell; GridCell { modelData: slot.modelData.e; at: slot.modelData.i; pane: grid.pane; view: grid } }
                    Component { id: cardCell; CardCell { modelData: slot.modelData.e; at: slot.modelData.i; pane: grid.pane; view: grid } }
                }
            }
        }
    }
}
