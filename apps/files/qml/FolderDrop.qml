// Somewhere files can be dropped: `dest`, a folder or "trash:". `over` is
// true while something that can go there is held over it; a drop gives
// `filesDropped(paths, dest)` and the window decides copy or move (Drops.plan).
import QtQuick
import "Drops.js" as D

DropArea {
    id: area
    property string dest
    property bool over: false
    signal filesDropped(var paths, string dest)
    keys: ["text/uri-list"]
    onEntered: drag => {
        // From another app the list may only arrive with the drop.
        const p = D.paths(drag.urls);
        const ok = area.dest !== "" && (p.length === 0 || D.accepts(p, area.dest));
        drag.accepted = ok;
        area.over = ok;
    }
    onExited: area.over = false
    onDropped: drop => {
        area.over = false;
        const p = D.paths(drop.urls);
        if (p.length === 0 || area.dest === "") return;
        // We do the copying or moving; the source app is told "copied" so it
        // never deletes its originals over our move.
        drop.accept(Qt.CopyAction);
        area.filesDropped(p, area.dest);
    }
}
