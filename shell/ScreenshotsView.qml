// Island view: recent screenshots. Click one to copy it; hover for open and
// delete; drag one into another app to attach or upload it. The list comes from the compositor (the Screenshots folder).
import QtQuick
import Quickshell
import Quickshell.Widgets
import "DragData.js" as DragData

Item {
    id: root
    implicitWidth: 24 + grid.width + 24
    implicitHeight: header.height + 16 + (shots.length ? grid.height : 48) + 40
    Component.onCompleted: Compositor.screenshotList()

    readonly property var shots: Compositor.screenshots.slice(0, 12)
    readonly property int columns: 4
    readonly property real cellW: 140
    readonly property real cellH: 88

    Item {
        id: header
        x: 24; y: 16
        width: grid.width
        height: 30
        ViewTabs { anchors.verticalCenter: parent.verticalCenter }
        Rectangle {
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            width: folderRow.implicitWidth + 24; height: 30; radius: 10
            color: folderMouse.containsMouse ? Theme.surface2 : Theme.surface1
            Row {
                id: folderRow
                anchors.centerIn: parent
                spacing: 6
                Icon { anchors.verticalCenter: parent.verticalCenter; name: "folder"; size: 13 }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "Open folder"
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(12); weight: Font.Medium }
                }
            }
            MouseArea {
                id: folderMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: {
                    const p = root.shots[0] ?? "";
                    Compositor.launch("", ["xdg-open", p ? p.slice(0, p.lastIndexOf("/")) : (Quickshell.env("HOME") + "/Pictures/Screenshots")]);
                    ShellState.close();
                }
            }
        }
    }

    Grid {
        id: grid
        x: 24
        y: header.y + header.height + 12
        columns: root.columns
        spacing: 10
        width: root.columns * root.cellW + (root.columns - 1) * 10
        Repeater {
            model: root.shots
            delegate: ClippingRectangle {
                id: cell
                required property string modelData
                width: root.cellW; height: root.cellH; radius: 10
                color: Theme.surface1
                border { width: cellMouse.containsMouse ? 1.5 : 0; color: Theme.pearl }
                Image {
                    anchors.fill: parent
                    source: "file://" + cell.modelData
                    fillMode: Image.PreserveAspectCrop
                    sourceSize: Qt.size(root.cellW * 2, root.cellH * 2)
                    asynchronous: true
                }
                MouseArea {
                    id: cellMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: drag.active ? Qt.ClosedHandCursor : Qt.PointingHandCursor
                    drag.target: ghost
                    onClicked: { Compositor.screenshotCopy(cell.modelData); ShellState.close(); }
                }
                DragGhost {
                    id: ghost
                    area: cellMouse
                    mime: DragData.forFiles([cell.modelData])
                    picture: cell
                    pictureScale: 0.75
                }
                Row {
                    anchors { right: parent.right; top: parent.top; margins: 6 }
                    spacing: 4
                    visible: cellMouse.containsMouse || openBtn.hovered || delBtn.hovered
                    IconButton {
                        id: openBtn
                        width: 24; height: 24; icon: "open"; iconSize: 12
                        fill: Qt.rgba(0.06, 0.06, 0.06, 0.85)
                        onClicked: { Compositor.launch("", ["xdg-open", cell.modelData]); ShellState.close(); }
                    }
                    IconButton {
                        id: delBtn
                        width: 24; height: 24; icon: "trash"; iconSize: 12
                        fill: Qt.rgba(0.06, 0.06, 0.06, 0.85); hoverFill: Theme.rose
                        iconColor: hovered ? Theme.bg : Theme.text
                        onClicked: Compositor.deleteScreenshot(cell.modelData)
                    }
                }
            }
        }
    }

    Text {
        anchors.horizontalCenter: grid.horizontalCenter
        y: grid.y + 12
        visible: root.shots.length === 0
        text: "No screenshots yet"
        color: Theme.faint
        font { family: Theme.font; pointSize: Theme.pt(13) }
    }
}
