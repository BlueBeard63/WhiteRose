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

    // A soft colour per kind of thing (folders: the desktop's accent), of
    // about the same lightness, mixed into the window's background so blocks
    // sit quietly; `t` is how much of the colour shows.
    function hue(e) {
        if (e.dir) return Theme.rose;
        const m = String(e.mime);
        return Qt.color(m.startsWith("image/") ? "#A78BDA" : m.startsWith("video/") ? "#62BDBD" : m.startsWith("audio/") ? "#E3A869"
            : /pdf|document|text\/plain|markdown|spreadsheet|presentation/.test(m) ? "#7FA7E0"
            : /zip|tar|7z|rar|compress|xz|gzip|zstd/.test(m) ? "#D9C27A"
            : /^text\/|json|javascript|x-shellscript|toml|yaml/.test(m) ? "#7DC1A1" : "#9AA3AD");
    }
    function mix(c, t) {
        const b = Theme.bg;
        return Qt.rgba(c.r * t + b.r * (1 - t), c.g * t + b.g * (1 - t), c.b * t + b.b * (1 - t), 1);
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
                readonly property color hue: root.hue(e)
                color: root.mix(hue, blockArea.containsMouse || drop.over ? 0.42 : 0.28)
                border { width: on ? 2 : 1; color: on ? Theme.rose : root.mix(hue, 0.6) }
                Behavior on color { ColorAnimation { duration: 100 } }
                clip: true
                Column {
                    visible: block.width > 64 && block.height > 34
                    x: 8; y: 6
                    width: block.width - 16
                    Row {
                        width: parent.width
                        spacing: 5
                        Icon { visible: block.e.dir; name: "folder"; size: 12; color: Theme.text; anchors.verticalCenter: parent.verticalCenter }
                        Text {
                        width: parent.width - (block.e.dir ? 17 : 0)
                        text: block.e.name
                        elide: Text.ElideMiddle
                        color: Theme.text
                        font { family: Theme.font; pointSize: Theme.pt(11.5); weight: Font.Medium }
                        }
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
