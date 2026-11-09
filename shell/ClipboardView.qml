// Island view: clipboard history. Click an entry to put it back on the
// clipboard. Copied files are re-checked on disk every time this opens:
// removed ones are marked, and an entry whose files are all gone can't be
// pasted. Clear removes the whole history (on disk too). Entries can be
// dragged into other apps: files and images as files, text as text.
import QtQuick
import Quickshell
import Quickshell.Widgets
import "DragData.js" as DragData

Item {
    id: root
    implicitWidth: 520
    implicitHeight: header.height + 16 + list.height + 40
    Component.onCompleted: Compositor.clipboardList()

    readonly property var entries: Compositor.clipboard

    function ago(t) {
        const s = Math.max(0, Math.floor(Date.now() / 1000) - t);
        if (s < 60) return "now";
        if (s < 3600) return Math.floor(s / 60) + "m";
        if (s < 86400) return Math.floor(s / 3600) + "h";
        return Math.floor(s / 86400) + "d";
    }

    Item {
        id: header
        x: 24; y: 16
        width: parent.width - 48
        height: 30
        ViewTabs { anchors.verticalCenter: parent.verticalCenter }
        Rectangle {
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            visible: root.entries.length > 0
            width: clearText.implicitWidth + 24; height: 30; radius: 10
            color: clearMouse.containsMouse ? Qt.rgba(0.91, 0.64, 0.68, 0.18) : Theme.surface1
            Text {
                id: clearText
                anchors.centerIn: parent
                text: "Clear"
                color: clearMouse.containsMouse ? Theme.rose : Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(12); weight: Font.Medium }
            }
            MouseArea {
                id: clearMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: Compositor.clipboardClear()
            }
        }
    }

    ListView {
        id: list
        x: 24
        y: header.y + header.height + 12
        width: parent.width - 48
        height: root.entries.length === 0 ? 48 : Math.min(contentHeight, 380)
        clip: true
        spacing: 4
        boundsBehavior: Flickable.StopAtBounds
        model: root.entries

        delegate: Rectangle {
            id: row
            required property var modelData
            readonly property var e: modelData
            width: list.width
            height: body.implicitHeight + 20
            radius: 12
            color: rowMouse.containsMouse && e.available ? Theme.surface1 : "transparent"
            opacity: e.available ? 1 : 0.5

            MouseArea {
                id: rowMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: drag.active ? Qt.ClosedHandCursor : row.e.available ? Qt.PointingHandCursor : Qt.ArrowCursor
                drag.target: ghost.mime ? ghost : null
                preventStealing: true // a drag out, not a scroll of the list (the wheel scrolls)
                onPressed: if (row.e.kind === "image" && ghost.mime) Compositor.clipboardPrepareDrag(row.e.id)
                onClicked: if (row.e.available) { Compositor.clipboardPaste(row.e.id); ShellState.close(); }
            }
            DragGhost {
                id: ghost
                area: rowMouse
                mime: DragData.forEntry(row.e)
                picture: row.e.kind === "image" ? thumb : body
                pictureScale: row.e.kind === "image" ? 1 : 0.7
            }

            Row {
                id: body
                x: 12
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - 24 - 28
                spacing: 12

                Rectangle {
                    visible: row.e.kind !== "image"
                    width: 30; height: 30; radius: 9
                    color: Theme.surface2
                    Icon { anchors.centerIn: parent; size: 15; color: Theme.pearl; name: row.e.kind === "files" ? "folder" : "clipboard" }
                }
                ClippingRectangle {
                    id: thumb
                    visible: row.e.kind === "image"
                    width: 120; height: 72; radius: 8
                    color: Theme.surface1
                    Image {
                        anchors.fill: parent
                        source: row.e.image ? "file://" + row.e.image : ""
                        fillMode: Image.PreserveAspectCrop
                        sourceSize: Qt.size(240, 144)
                        asynchronous: true
                    }
                }
                Column {
                    anchors.verticalCenter: parent.verticalCenter
                    width: body.width - (row.e.kind === "image" ? 120 : 30) - 12 - 32
                    spacing: 2
                    Text {
                        visible: row.e.kind === "text"
                        width: parent.width
                        text: row.e.preview.replace(/\s+/g, " ").trim()
                        maximumLineCount: 2
                        wrapMode: Text.Wrap
                        elide: Text.ElideRight
                        color: Theme.text
                        font { family: Theme.font; pointSize: Theme.pt(12.5) }
                    }
                    Text {
                        visible: row.e.kind === "image"
                        text: "Image"
                        color: Theme.text
                        font { family: Theme.font; pointSize: Theme.pt(12.5); weight: Font.Medium }
                    }
                    Repeater {
                        model: row.e.kind === "files" ? row.e.files.slice(0, 3) : []
                        delegate: Row {
                            required property var modelData
                            spacing: 6
                            Text {
                                text: modelData.path.slice(modelData.path.lastIndexOf("/") + 1)
                                color: modelData.exists ? Theme.text : Theme.faint
                                font { family: Theme.font; pointSize: Theme.pt(12.5); strikeout: !modelData.exists }
                                elide: Text.ElideMiddle
                                width: Math.min(implicitWidth, 300)
                            }
                            Text {
                                visible: !modelData.exists
                                text: "missing"
                                color: Theme.rose
                                font { family: Theme.font; pointSize: Theme.pt(11) }
                            }
                        }
                    }
                    Text {
                        visible: row.e.kind === "files" && row.e.files.length > 3
                        text: `+${row.e.files.length - 3} more`
                        color: Theme.muted
                        font { family: Theme.font; pointSize: Theme.pt(11) }
                    }
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: root.ago(row.e.time)
                    color: Theme.faint
                    font { family: Theme.font; pointSize: Theme.pt(11) }
                }
            }

            // Remove one entry.
            IconButton {
                anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
                visible: rowMouse.containsMouse || hovered
                width: 24; height: 24
                icon: "x"; iconSize: 12
                fill: Theme.surface2; hoverFill: Theme.rose
                iconColor: hovered ? Theme.bg : Theme.text
                onClicked: Compositor.clipboardDelete(row.e.id)
            }
        }
    }

    Text {
        anchors.horizontalCenter: list.horizontalCenter
        y: list.y + 12
        visible: root.entries.length === 0
        text: "Nothing copied yet"
        color: Theme.faint
        font { family: Theme.font; pointSize: Theme.pt(13) }
    }
}
