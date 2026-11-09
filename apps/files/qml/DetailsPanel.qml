// Beside the folder (Alt+P): the selected file previewed, with what it is.
// Several selected: how many and their size; none: the folder itself.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F

Rectangle {
    id: root
    property var pane: null
    readonly property var sel: root.pane?.selection ?? []
    readonly property var entry: root.sel.length === 1 ? root.sel[0] : null
    readonly property real now: Date.now()
    color: Theme.bg
    Rectangle { width: 1; height: parent.height; color: Theme.surface1 }

    component Fact: Column {
        property string label
        property string value
        visible: value !== ""
        width: parent.width
        spacing: 1
        Text { text: parent.label; color: Theme.faint; font { family: Theme.font; pointSize: Theme.pt(10.5) } }
        Text {
            width: parent.width
            text: parent.value
            wrapMode: Text.WrapAnywhere
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(12) }
        }
    }

    Flickable {
        anchors { fill: parent; leftMargin: 1 }
        contentHeight: col.implicitHeight + 32
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        Column {
            id: col
            x: 16; y: 16
            width: parent.width - 32
            spacing: 14

            // ---- one file ------------------------------------------------------------
            Rectangle {
                visible: root.entry !== null
                width: parent.width; height: Math.round(width * 0.8)
                radius: 12
                color: Theme.surface1
                clip: true
                Preview {
                    id: preview
                    anchors { fill: parent; margins: 6 }
                    entry: root.entry
                    visible: !root.entry?.look
                }
                FolderIcon {
                    visible: !!root.entry?.look
                    anchors.centerIn: parent
                    size: Math.min(parent.width, parent.height) * 0.5
                    look: root.entry?.look ?? ({})
                }
            }
            Column {
                visible: root.entry !== null
                width: parent.width
                spacing: 10
                Text {
                    width: parent.width
                    text: root.entry?.name ?? ""
                    wrapMode: Text.WrapAnywhere
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(13.5); weight: Font.DemiBold }
                }
                Fact { label: "Kind"; value: root.entry ? (root.entry.dir ? "Folder" : F.kind(root.entry.mime)) : "" }
                Fact {
                    label: preview.kind === "image" ? "Dimensions" : preview.kind === "pdf" ? "Pages" : preview.kind === "video" || preview.kind === "audio" ? "Length" : ""
                    value: label !== "" ? preview.facts : ""
                }
                Fact { label: "Size"; value: root.entry && !root.entry.dir ? F.exactSize(root.entry.size) : "" }
                Fact { label: root.pane?.inTrash ? "Deleted" : "Modified"; value: root.entry ? F.fullDate(root.entry.modified) : "" }
                Fact { label: root.pane?.inTrash ? "From" : "Folder"; value: String(root.entry?.origin ?? root.entry?.folder ?? "").replace(Fs.home(), "~") }
                Button { text: "Open"; kind: "primary"; anchors.verticalCenter: undefined; onClicked: root.pane.activate(root.entry) }
            }

            // ---- several, or none (the folder) ------------------------------------------------
            Column {
                visible: root.entry === null
                width: parent.width
                spacing: 10
                Icon { name: root.sel.length > 1 ? "copy" : root.pane?.searching ? "search" : root.pane?.inDrives ? "gauge" : root.pane?.inTrash ? "trash" : "folder"; size: 40; color: Theme.muted }
                Text {
                    width: parent.width
                    text: root.sel.length > 1 ? root.sel.length + " items" : root.pane?.title ?? ""
                    wrapMode: Text.WrapAnywhere
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(13.5); weight: Font.DemiBold }
                }
                Fact {
                    label: "Size"
                    value: {
                        const files = root.sel.filter(e => !e.dir);
                        return root.sel.length > 1 && files.length > 0 ? F.size(files.reduce((a, e) => a + e.size, 0)) + (files.length < root.sel.length ? " (files only)" : "") : "";
                    }
                }
                Fact {
                    label: "Contains"
                    value: root.sel.length === 0 && root.pane ? root.pane.shownEntries.length + (root.pane.shownEntries.length === 1 ? " item" : " items") : ""
                }
            }
        }
    }
}
