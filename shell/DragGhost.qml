// What an island item drags out into other apps. Point a MouseArea's
// drag.target at this (when `mime` isn't null); once the press moves past the
// drag threshold, Qt starts a real Wayland drag carrying `mime`, with a
// snapshot of `picture` under the pointer. A finished drop closes the island;
// a cancelled one leaves it open.
import QtQuick

Item {
    id: ghost
    required property MouseArea area
    // Drag.mimeData map (see DragData.js); null means not draggable.
    property var mime: null
    property Item picture: null
    property real pictureScale: 1

    width: 1; height: 1
    opacity: 0

    Drag.active: area.drag.active
    Drag.dragType: Drag.Automatic
    Drag.supportedActions: Qt.CopyAction
    Drag.mimeData: mime ?? {}
    Drag.hotSpot: Qt.point(12, 12)
    Drag.onDragStarted: ShellState.dragging = true
    Drag.onDragFinished: dropAction => {
        ShellState.dragging = false;
        x = 0; y = 0;
        if (dropAction !== Qt.IgnoreAction) ShellState.close();
    }

    // Snapshot the picture on press, so the icon is ready when the drag starts.
    Connections {
        target: ghost.area
        function onPressed() {
            const p = ghost.picture;
            if (!ghost.mime || !p) return;
            p.grabToImage(r => ghost.Drag.imageSource = r.url,
                Qt.size(Math.round(p.width * ghost.pictureScale), Math.round(p.height * ghost.pictureScale)));
        }
    }
}
