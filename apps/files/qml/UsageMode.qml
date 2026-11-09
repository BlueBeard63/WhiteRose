// Disk usage: what's in the folder as a map, each block sized by how much
// space it takes (folders measured in the background, filling in as they
// come). Click selects, double-click opens (a folder: in you go).
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F
import "Layout.js" as Layout

FocusScope {
    id: root
    required property var pane
    focus: true
    // Each item and what it weighs (folders: once measured).
    readonly property var weighed: root.pane.shownEntries.map(e => ({
        entry: e,
        value: e.dir ? (root.pane.sizes[e.path]?.bytes ?? 0) : e.size,
    }))
    readonly property real total: root.weighed.reduce((a, w) => a + w.value, 0)
    readonly property int waiting: root.pane.shownEntries.filter(e => e.dir && !e.link && !root.pane.sizes[e.path]).length
    readonly property var blocks: Layout.treemap(root.weighed, 0, 0, map.width, map.height)
    property string hover: ""

    // A colour per kind of thing.
    function shade(e) {
        if (e.dir) return Theme.tint(0.55);
        const m = String(e.mime);
        const c = m.startsWith("image/") ? "#986EE2" : m.startsWith("video/") ? "#E5534B" : m.startsWith("audio/") ? "#F0883E"
            : /pdf|document|text\/plain|markdown|spreadsheet|presentation/.test(m) ? "#539BF5"
            : /zip|tar|7z|rar|compress|xz|gzip|zstd/.test(m) ? "#E3B341"
            : /^text\/|json|javascript|x-shellscript|toml|yaml/.test(m) ? "#57AB5A" : "#8B949E";
        return Qt.rgba(Qt.color(c).r, Qt.color(c).g, Qt.color(c).b, 0.55);
    }

    Keys.onReturnPressed: root.pane.activate(root.pane.shownEntries[root.pane.current])

    Item {
        id: map
        anchors { left: parent.left; right: parent.right; top: parent.top; bottom: foot.top; margins: 12 }
        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            onClicked: m => {
                root.pane.selected = {};
                root.forceActiveFocus();
                if (m.button === Qt.RightButton) { const p = mapToItem(root.pane, m.x, m.y); root.pane.menuAt(null, p.x, p.y) }
            }
        }
        Repeater {
            model: root.blocks
            delegate: Rectangle {
                id: block
                required property var modelData
                readonly property var e: modelData.item.entry
                readonly property bool on: !!root.pane.selected[e.path]
                x: modelData.x + 1; y: modelData.y + 1
                width: Math.max(0, modelData.w - 2); height: Math.max(0, modelData.h - 2)
                radius: Math.min(8, width / 4, height / 4)
                color: root.shade(e)
                border { width: on || drop.over ? 2 : 0; color: Theme.text }
                opacity: blockArea.containsMouse ? 1 : 0.88
                clip: true
                Column {
                    visible: block.width > 64 && block.height > 34
                    x: 8; y: 6
                    width: block.width - 16
                    Text {
                        width: parent.width
                        text: block.e.name
                        elide: Text.ElideMiddle
                        color: Theme.text
                        font { family: Theme.font; pointSize: Theme.pt(11.5); weight: Font.Medium }
                    }
                    Text {
                        visible: block.height > 52
                        text: F.size(block.modelData.item.value) + " · " + Math.round(100 * block.modelData.item.value / Math.max(1, root.total)) + "%"
                        color: Theme.muted
                        font { family: Theme.font; pointSize: Theme.pt(10.5) }
                    }
                }
                FolderDrop { id: drop; anchors.fill: parent; enabled: block.e.dir; dest: block.e.path; onFilesDropped: (p, d) => root.pane.dropped(p, d) }
                MouseArea {
                    id: blockArea
                    anchors.fill: parent
                    hoverEnabled: true
                    acceptedButtons: Qt.LeftButton | Qt.RightButton
                    onContainsMouseChanged: root.hover = containsMouse ? block.e.name + " · " + F.size(block.modelData.item.value) : ""
                    onClicked: m => {
                        root.forceActiveFocus();
                        const i = root.pane.shownEntries.indexOf(block.e);
                        if (m.button === Qt.RightButton) {
                            if (!block.on) root.pane.clickAt(i, false, false);
                            const p = mapToItem(root.pane, m.x, m.y);
                            root.pane.menuAt(block.e, p.x, p.y);
                            return;
                        }
                        root.pane.clickAt(i, m.modifiers & Qt.ControlModifier, m.modifiers & Qt.ShiftModifier);
                    }
                    onDoubleClicked: root.pane.activate(block.e)
                }
            }
        }
        Text {
            anchors.centerIn: parent
            visible: root.blocks.length === 0 && root.pane.shownEntries.length > 0
            text: root.waiting > 0 ? "Measuring…" : "Nothing here takes up space"
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(13) }
        }
    }
    Text {
        id: foot
        anchors { left: parent.left; leftMargin: 14; right: parent.right; rightMargin: 14; bottom: parent.bottom; bottomMargin: 8 }
        text: root.hover !== "" ? root.hover
            : F.size(root.total) + " in all" + (root.waiting > 0 ? " · measuring " + root.waiting + (root.waiting === 1 ? " folder…" : " folders…") : "")
        elide: Text.ElideMiddle
        color: Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(11.5) }
    }
}
