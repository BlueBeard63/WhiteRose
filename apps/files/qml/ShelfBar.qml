// The shelf (shelf.rs): files gathered from anywhere, along the bottom of
// the window while there's anything on it. Drop files on it (or "Put on
// the shelf", Ctrl+Shift+S) to gather them; drag them off it anywhere, or
// copy or move them all into the folder you're in. Every window shares it.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Drops.js" as D

Rectangle {
    id: root
    property var items: []
    // Where "Copy here" / "Move here" put them ("" when that's not a folder).
    property string here: ""
    signal copyHere(var paths)
    signal moveHere(var paths)
    signal said(string text)
    function refresh() { root.items = JSON.parse(Fs.shelf()) }
    function add(paths) {
        const n = Fs.shelfAdd(JSON.stringify(paths));
        root.refresh();
        root.said(n === 0 ? "Already on the shelf" : n === 1 ? "Put on the shelf" : n + " put on the shelf");
    }
    Component.onCompleted: refresh()
    // Another window may have changed it.
    Connections { target: Qt.application; function onStateChanged() { if (Qt.application.state === Qt.ApplicationActive) root.refresh() } }

    readonly property bool dropping: drop.containsDrag
    height: root.items.length > 0 || root.dropping ? 64 : 0
    visible: height > 0
    clip: true
    color: drop.containsDrag ? Theme.tint(0.12) : Theme.bg
    Behavior on height { NumberAnimation { duration: 140; easing.type: Easing.OutCubic } }
    Rectangle { width: parent.width; height: 1; color: Theme.surface1 }

    // Files held over the window can be dropped here.
    DropArea {
        id: drop
        anchors.fill: parent
        keys: ["text/uri-list"]
        onDropped: d => {
            const p = D.paths(d.urls);
            if (p.length === 0) return;
            d.accept(Qt.CopyAction);
            root.add(p);
        }
    }

    Row {
        id: label
        anchors { left: parent.left; leftMargin: 16; verticalCenter: parent.verticalCenter }
        spacing: 6
        Icon { anchors.verticalCenter: parent.verticalCenter; name: "briefcase"; size: 14; color: Theme.muted }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.items.length === 0 ? "Drop here to put on the shelf" : "Shelf · " + root.items.length
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(11.5); weight: Font.Medium }
        }
    }

    ListView {
        id: list
        anchors { left: label.right; leftMargin: 14; right: actions.left; rightMargin: 10; top: parent.top; bottom: parent.bottom; topMargin: 8; bottomMargin: 8 }
        orientation: ListView.Horizontal
        spacing: 6
        clip: true
        model: root.items
        boundsBehavior: Flickable.StopAtBounds
        delegate: Rectangle {
            id: chip
            required property var modelData
            width: Math.min(200, chipRow.implicitWidth + 20); height: 48; radius: 10
            color: chipArea.containsMouse ? Theme.surface2 : Theme.surface1
            Row {
                id: chipRow
                x: 8; anchors.verticalCenter: parent.verticalCenter
                spacing: 8
                Image { width: 28; height: 28; sourceSize: Qt.size(56, 56); source: chip.modelData.icon ? "file://" + chip.modelData.icon : ""; anchors.verticalCenter: parent.verticalCenter }
                Text {
                    width: Math.min(implicitWidth, 130)
                    anchors.verticalCenter: parent.verticalCenter
                    elide: Text.ElideMiddle
                    text: chip.modelData.name
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(11.5) }
                }
            }
            // Off the shelf (stays where it is on disk).
            IconButton {
                visible: chipArea.containsMouse || hovered
                anchors { right: parent.right; top: parent.top; margins: 2 }
                width: 18; height: 18; icon: "x"; iconSize: 9
                onClicked: { Fs.shelfRemove(JSON.stringify([chip.modelData.path])); root.refresh() }
            }
            // Dragged off: to a folder, another app, the desktop.
            DragSource { id: carry; paths: [chip.modelData.path]; icon: chip.modelData.icon ?? "" }
            MouseArea {
                id: chipArea
                anchors.fill: parent
                z: -1
                hoverEnabled: true
                onPressed: m => carry.press(m.x, m.y)
                onPositionChanged: m => { if (pressed) carry.moved(m.x, m.y) }
                ToolTip.visible: containsMouse
                ToolTip.delay: 600
                ToolTip.text: chip.modelData.path.replace(Fs.home(), "~")
            }
        }
    }

    Row {
        id: actions
        anchors { right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
        spacing: 6
        visible: root.items.length > 0
        // All of it at once: dragged, or copied / moved here.
        Rectangle {
            width: allText.implicitWidth + 20; height: 30; radius: 9
            color: allArea.containsMouse ? Theme.surface2 : Theme.surface1
            Text { id: allText; anchors.centerIn: parent; text: "Drag all"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(11.5); weight: Font.Medium } }
            DragSource { id: carryAll; paths: root.items.map(i => i.path); icon: root.items[0]?.icon ?? "" }
            MouseArea {
                id: allArea
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.OpenHandCursor
                onPressed: m => carryAll.press(m.x, m.y)
                onPositionChanged: m => { if (pressed) carryAll.moved(m.x, m.y) }
            }
        }
        Button { text: "Copy here"; enabled: root.here !== ""; onClicked: root.copyHere(root.items.map(i => i.path)) }
        Button {
            text: "Move here"; enabled: root.here !== ""
            onClicked: { root.moveHere(root.items.map(i => i.path)); Fs.shelfClear(); root.refresh() }
        }
        Button { text: "Clear"; kind: "ghost"; onClicked: { Fs.shelfClear(); root.refresh() } }
    }
}
