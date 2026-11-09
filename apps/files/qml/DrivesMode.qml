// Storage: every mounted drive as a card, how full it is, and what's free.
// Click to open one; right-click for more.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F

GridView {
    id: grid
    required property var pane
    model: grid.pane.shownEntries
    // As many 300+ px cards as fit across (inside the margins).
    readonly property real inner: width - leftMargin - rightMargin
    cellWidth: inner / Math.max(1, Math.floor(inner / 300))
    cellHeight: 112
    clip: true
    focus: true
    boundsBehavior: Flickable.StopAtBounds
    leftMargin: 12; rightMargin: 12; topMargin: 12; bottomMargin: 12
    ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
    currentIndex: grid.pane.current
    keyNavigationEnabled: true
    highlightFollowsCurrentItem: false
    onCurrentIndexChanged: if (currentIndex !== grid.pane.current) grid.pane.current = currentIndex
    Keys.onReturnPressed: grid.pane.activate(grid.pane.shownEntries[grid.pane.current])

    delegate: Item {
        id: cell
        required property var modelData
        required property int index
        readonly property real used: modelData.total > 0 ? (modelData.total - modelData.free) / modelData.total : 0
        width: grid.cellWidth; height: grid.cellHeight
        Rectangle {
            anchors { fill: parent; margins: 6 }
            radius: 14
            color: area.containsMouse ? Theme.surface2 : Theme.surface1
            border { width: cell.index === grid.currentIndex && grid.activeFocus ? 1 : 0; color: Theme.tint(0.6) }
            Image {
                id: ic
                anchors { left: parent.left; leftMargin: 16; verticalCenter: parent.verticalCenter }
                width: 44; height: 44
                sourceSize: Qt.size(88, 88)
                source: cell.modelData.icon ? "file://" + cell.modelData.icon : ""
            }
            Column {
                anchors { left: ic.right; leftMargin: 14; right: parent.right; rightMargin: 16; verticalCenter: parent.verticalCenter }
                spacing: 6
                Text {
                    width: parent.width
                    text: cell.modelData.name
                    elide: Text.ElideRight
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.Medium }
                }
                Rectangle {
                    width: parent.width; height: 6; radius: 3
                    color: Theme.surface2
                    Rectangle {
                        width: parent.width * Math.min(1, cell.used); height: 6; radius: 3
                        // Nearly full: in rose.
                        color: cell.used > 0.9 ? Theme.rose : Theme.muted
                    }
                }
                Text {
                    width: parent.width
                    text: cell.modelData.total > 0 ? F.size(cell.modelData.free) + " free of " + F.size(cell.modelData.total) : cell.modelData.path
                    elide: Text.ElideMiddle
                    color: Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(11.5) }
                }
            }
            MouseArea {
                id: area
                anchors.fill: parent
                hoverEnabled: true
                acceptedButtons: Qt.LeftButton | Qt.RightButton
                onClicked: m => {
                    grid.pane.clickAt(cell.index, false, false);
                    grid.forceActiveFocus();
                    if (m.button === Qt.RightButton) { const p = mapToItem(grid.pane, m.x, m.y); grid.pane.menuAt(cell.modelData, p.x, p.y) }
                    else grid.pane.activate(cell.modelData);
                }
            }
        }
    }
}
