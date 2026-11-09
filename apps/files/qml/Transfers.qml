// Copies, moves, trashing and deleting as they happen, one card each:
// how far, how fast, how long left, pause and cancel. A clash asks here
// (what's there against what's coming, replace / keep both / skip, or the
// same for every clash). Finished cards go by themselves unless something
// went wrong; trashing that failed can delete instead.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F

Column {
    id: root
    readonly property var jobs: JSON.parse(Ops.jobs || "[]")
    readonly property real now: Date.now()
    // Asked to delete what couldn't go to the Trash.
    signal deleteInstead(var paths)
    spacing: 8
    width: 380

    Repeater {
        model: root.jobs
        delegate: Rectangle {
            id: card
            required property var modelData
            readonly property var job: modelData
            readonly property bool done: job.state === "done"
            readonly property bool ok: done && job.failed.length === 0 && !job.cancelled
            readonly property bool transfer: job.kind === "copy" || job.kind === "move"
            readonly property real fraction: job.bytesTotal > 0 ? job.bytesDone / job.bytesTotal
                : job.itemsTotal > 0 ? job.itemsDone / job.itemsTotal : 0
            property bool forAll: false
            width: root.width
            height: body.implicitHeight + 24
            radius: 14
            color: Theme.surface1
            border { width: 1; color: Theme.surface2 }

            // Done and fine: gone in a moment.
            Timer { running: card.ok; interval: 2500; onTriggered: Ops.dismiss(card.job.id) }

            Column {
                id: body
                x: 14; y: 12
                width: parent.width - 28
                spacing: 8

                // ---- what, and its buttons ---------------------------------------------
                Item {
                    width: parent.width; height: 24
                    Text {
                        anchors { left: parent.left; right: buttons.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
                        text: card.ok ? card.job.label.replace(/^Copying/, "Copied").replace(/^Moving/, "Moved").replace(/^Deleting/, "Deleted")
                                      : card.job.label
                        elide: Text.ElideMiddle
                        color: Theme.text
                        font { family: Theme.font; pointSize: Theme.pt(12.5); weight: Font.Medium }
                    }
                    Row {
                        id: buttons
                        anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                        spacing: 2
                        IconButton {
                            visible: card.transfer && (card.job.state === "running" || card.job.state === "paused")
                            width: 24; height: 24; iconSize: 12; fill: "transparent"
                            icon: card.job.state === "paused" ? "play" : "pause"
                            onClicked: Ops.pause(card.job.id, card.job.state !== "paused")
                        }
                        IconButton {
                            width: 24; height: 24; iconSize: 12; fill: "transparent"
                            icon: "x"
                            onClicked: card.done ? Ops.dismiss(card.job.id) : Ops.cancel(card.job.id)
                        }
                    }
                }

                // ---- progress ------------------------------------------------------------
                Rectangle {
                    visible: !card.done
                    width: parent.width; height: 4; radius: 2
                    color: Theme.surface2
                    Rectangle {
                        width: parent.width * Math.min(1, card.fraction); height: 4; radius: 2
                        color: card.job.state === "paused" ? Theme.muted : Theme.rose
                        Behavior on width { NumberAnimation { duration: 120 } }
                    }
                }
                Text {
                    width: parent.width
                    visible: text !== ""
                    text: {
                        const j = card.job;
                        if (j.state === "waiting") return "Waiting for the one before it";
                        if (j.state === "paused") return "Paused · " + F.size(j.bytesDone) + " of " + F.size(j.bytesTotal);
                        if (j.state === "conflict") return "";
                        if (card.done) {
                            if (j.cancelled) return "Stopped";
                            if (j.failed.length === 0) return "";
                            const first = j.failed[0];
                            const name = first[0].substring(first[0].lastIndexOf("/") + 1);
                            return "“" + name + "”: " + first[1] + (j.failed.length > 1 ? ", and " + (j.failed.length - 1) + " more" : "");
                        }
                        if (j.bytesTotal > 0 && card.transfer) {
                            const parts = [F.size(j.bytesDone) + " of " + F.size(j.bytesTotal)];
                            if (j.rate > 0) parts.push(F.size(j.rate) + "/s", F.eta(j.bytesTotal - j.bytesDone, j.rate));
                            return parts.filter(p => p).join(" · ");
                        }
                        return j.itemsDone + " of " + j.itemsTotal + " items";
                    }
                    wrapMode: Text.Wrap
                    color: card.done && card.job.failed.length > 0 ? Theme.rose : Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(11.5) }
                }
                Button {
                    visible: card.done && card.job.kind === "trash" && card.job.failed.length > 0
                    text: "Delete permanently instead"
                    kind: "danger"
                    anchors.verticalCenter: undefined
                    onClicked: { root.deleteInstead(card.job.failed.map(f => f[0])); Ops.dismiss(card.job.id) }
                }

                // ---- a clash -------------------------------------------------------------
                Column {
                    visible: card.job.state === "conflict" && !!card.job.conflict
                    width: parent.width
                    spacing: 8
                    readonly property var c: card.job.conflict ?? ({ src: "", dst: "", folders: false })
                    readonly property string name: c.dst.substring(c.dst.lastIndexOf("/") + 1)
                    readonly property string where: { const d = c.dst.substring(0, c.dst.lastIndexOf("/")); return d.substring(d.lastIndexOf("/") + 1) || "/" }
                    Text {
                        width: parent.width
                        text: (parent.c.folders ? "There's already a folder “" : "“") + parent.name + (parent.c.folders ? "” in " : "” is already in ") + parent.where
                        wrapMode: Text.Wrap
                        color: Theme.text
                        font { family: Theme.font; pointSize: Theme.pt(12) }
                    }
                    // Side by side: what's coming and what's there.
                    Row {
                        visible: !parent.c.folders
                        width: parent.width
                        spacing: 8
                        Repeater {
                            model: [
                                { title: "Coming", size: parent.parent.c.srcSize, modified: parent.parent.c.srcModified },
                                { title: "Already there", size: parent.parent.c.dstSize, modified: parent.parent.c.dstModified },
                            ]
                            delegate: Rectangle {
                                required property var modelData
                                width: (parent.width - 8) / 2; height: 50; radius: 10
                                color: Theme.bg
                                Column {
                                    x: 10; anchors.verticalCenter: parent.verticalCenter
                                    spacing: 2
                                    Text { text: modelData.title; color: Theme.faint; font { family: Theme.font; pointSize: Theme.pt(10.5) } }
                                    Text { text: F.size(modelData.size) + " · " + F.date(modelData.modified, root.now); color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
                                }
                            }
                        }
                    }
                    Row {
                        spacing: 8
                        Toggle { checked: card.forAll; onToggled: on => card.forAll = on }
                        Text { anchors.verticalCenter: parent.verticalCenter; text: "Same for the rest"; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
                    }
                    Row {
                        anchors.right: parent.right
                        spacing: 6
                        Button { text: "Skip"; kind: "ghost"; onClicked: Ops.answer(card.job.id, "skip", card.forAll) }
                        Button { text: "Keep both"; onClicked: Ops.answer(card.job.id, "keep", card.forAll) }
                        Button { text: parent.parent.c.folders ? "Merge" : "Replace"; kind: "primary"; onClicked: Ops.answer(card.job.id, "replace", card.forAll) }
                    }
                }
            }
        }
    }
}
