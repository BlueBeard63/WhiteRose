// Carries files out of a view. The item's MouseArea calls press() and
// moved(); past a few pixels the drag starts: the paths go as a file list
// any app understands (text/uri-list, plus the paths as text), with the
// item's icon under the pointer. (Started by hand, not with the MouseArea's
// drag.target, which Qt mishandles once the item goes away mid-drag.)
import QtQuick
import "Drops.js" as D
import "Format.js" as Fmt

Item {
    id: src
    property var paths: []
    property string icon
    property point from
    width: 1; height: 1

    function press(x, y) { src.from = Qt.point(x, y) }
    function moved(x, y) {
        if (src.Drag.active || src.paths.length === 0) return;
        if (Math.abs(x - src.from.x) + Math.abs(y - src.from.y) < 10) return;
        src.Drag.active = true;
    }

    Drag.dragType: Drag.Automatic
    Drag.supportedActions: Qt.CopyAction | Qt.MoveAction | Qt.LinkAction
    Drag.proposedAction: Qt.MoveAction
    Drag.mimeData: ({ "text/uri-list": D.uris(src.paths), "text/plain": src.paths.join("\n") })
    Drag.imageSource: src.icon ? Fmt.fileUrl(src.icon) : ""
    Drag.imageSourceSize: Qt.size(48, 48)
    Drag.hotSpot: Qt.point(24, 24)
    Drag.onDragFinished: src.Drag.active = false
}
