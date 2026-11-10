// The Activity drawer, along the bottom of the window: copies, moves,
// trashing, deleting, extracting and compressing as they happen, a card
// each (how far, how fast, how long left; pause, cancel). A clash asks in
// its card: what's coming against what's there, for text files the
// changes line by line right there, and skip / keep both / keep newer /
// replace (or the same for every clash). Transfers that didn't finish last
// time can carry on from here. Finished cards go by themselves unless
// something went wrong; trashing that failed can delete instead. The
// header folds it away; it's gone when there's nothing to show.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F

Rectangle {
    id: root
    readonly property var jobs: JSON.parse(Ops.jobs || "[]")
    // Copies and moves that didn't finish last time (Main reads them).
    property var unfinished: []
    readonly property real now: Date.now()
    property bool folded: false
    readonly property int asking: root.jobs.filter(j => j.state === "conflict").length
    readonly property bool shown: root.jobs.length > 0 || root.unfinished.length > 0
    signal deleteInstead(var paths)
    signal forget(string key)
    signal carryOn(string key)

    // A clash needs you: unfolded.
    onAskingChanged: if (root.asking > 0) root.folded = false
    visible: root.shown
    height: !root.shown ? 0 : root.folded ? 34 : 34 + cards.height + 12
    color: Theme.bg
    clip: true
    Behavior on height { NumberAnimation { duration: 140; easing.type: Easing.OutCubic } }
    Rectangle { width: parent.width; height: 1; color: Theme.surface1 }

    readonly property color addInk: "#57AB5A"
    readonly property color addBg: Qt.rgba(0.34, 0.67, 0.35, 0.14)
    readonly property color delBg: Qt.rgba(0.9, 0.33, 0.29, 0.14)

    // ---- the header ----
    Item {
        id: head
        x: 16; width: parent.width - 32; height: 34
        Row {
            anchors.verticalCenter: parent.verticalCenter
            spacing: 10
            Text { text: "ACTIVITY"; color: Theme.faint; font { family: Theme.font; pointSize: Theme.pt(10); weight: Font.Medium; letterSpacing: 0.6 } }
            Text {
                text: {
                    const n = root.jobs.length + root.unfinished.length;
                    return n + (n === 1 ? " thing" : " things") + " going" + (root.asking > 0 ? " · " + root.asking + " " + (root.asking === 1 ? "needs" : "need") + " you" : "");
                }
                color: root.asking > 0 ? Theme.rose : Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(11) }
            }
        }
        IconButton {
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            width: 24; height: 24; iconSize: 12; fill: "transparent"
            icon: root.folded ? "chevUp" : "chevDown"
            onClicked: root.folded = !root.folded
        }
        MouseArea { anchors { fill: parent; rightMargin: 30 } z: -1; onClicked: root.folded = !root.folded }
    }

    // ---- the cards, side by side ----
    Flickable {
        id: cards
        anchors { top: head.bottom; left: parent.left; right: parent.right; leftMargin: 16; rightMargin: 16 }
        height: Math.min(260, row.height)
        contentWidth: row.width
        contentHeight: row.height
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        flickableDirection: Flickable.AutoFlickIfNeeded
        ScrollBar.horizontal: ScrollBar { policy: ScrollBar.AsNeeded }
        Row {
            id: row
            spacing: 10

            // ---- didn't finish last time ----
            Repeater {
                model: root.unfinished
                delegate: Rectangle {
                    id: uf
                    required property var modelData
                    width: 320; height: ufCol.implicitHeight + 24
                    radius: 12
                    color: Theme.surface1
                    Column {
                        id: ufCol
                        x: 14; y: 12; width: parent.width - 28
                        spacing: 6
                        Text {
                            width: parent.width
                            wrapMode: Text.Wrap
                            text: (uf.modelData.kind === "move" ? "Moving " : "Copying ") + uf.modelData.remaining.length + (uf.modelData.remaining.length === 1 ? " item" : " items")
                                + " to " + uf.modelData.dest.substring(uf.modelData.dest.lastIndexOf("/") + 1) + " didn't finish"
                            color: Theme.text
                            font { family: Theme.font; pointSize: Theme.pt(12); weight: Font.Medium }
                        }
                        Text {
                            width: parent.width
                            wrapMode: Text.Wrap
                            text: "Started " + F.date(uf.modelData.started * 1000, Date.now()) + ". Carrying on skips what's already there and finishes the rest."
                            color: Theme.muted
                            font { family: Theme.font; pointSize: Theme.pt(10.5) }
                        }
                        Row {
                            spacing: 6
                            Button { anchors.verticalCenter: undefined; text: "Carry on"; kind: "primary"; onClicked: root.carryOn(uf.modelData.key) }
                            Button { anchors.verticalCenter: undefined; text: "Forget it"; kind: "ghost"; onClicked: root.forget(uf.modelData.key) }
                        }
                    }
                }
            }

            // ---- what's going on now ----
            Repeater {
                model: root.jobs
                delegate: Rectangle {
                    id: card
                    required property var modelData
                    readonly property var job: modelData
                    readonly property bool done: job.state === "done"
                    readonly property bool ok: done && job.failed.length === 0 && !job.cancelled
                    readonly property bool transfer: job.kind === "copy" || job.kind === "move"
                    readonly property bool clash: job.state === "conflict" && !!job.conflict
                    readonly property var c: job.conflict ?? ({ src: "", dst: "", folders: false })
                    readonly property real fraction: job.bytesTotal > 0 ? job.bytesDone / job.bytesTotal
                        : job.itemsTotal > 0 ? job.itemsDone / job.itemsTotal : 0
                    property bool forAll: false
                    // A text file clashing: its changes, worked out once.
                    property var diff: null
                    property string diffFor: ""
                    function lookAtDiff() {
                        if (!card.clash || !card.c.text || card.diffFor === card.c.dst + "|" + card.c.src) return;
                        card.diffFor = card.c.dst + "|" + card.c.src;
                        card.diff = JSON.parse(Ops.diff(card.c.dst, card.c.src));
                    }
                    onClashChanged: lookAtDiff()
                    onCChanged: lookAtDiff()
                    Component.onCompleted: lookAtDiff()
                    readonly property bool wide: card.clash && !!card.diff?.ok && card.diff.hunks.length > 0
                    width: wide ? Math.min(560, root.width - 32) : 320
                    height: body.implicitHeight + 24
                    radius: 12
                    color: Theme.surface1
                    border { width: card.clash ? 1 : 0; color: Theme.tint(0.6) }

                    // Done and fine: gone in a moment.
                    Timer { running: card.ok; interval: 2500; onTriggered: Ops.dismiss(card.job.id) }

                    Column {
                        id: body
                        x: 14; y: 12
                        width: parent.width - 28
                        spacing: 7

                        // ---- what ----
                        Item {
                            width: parent.width; height: 20
                            Icon {
                                id: kindIcon
                                anchors.verticalCenter: parent.verticalCenter
                                name: card.clash ? "alert" : card.job.kind === "trash" || card.job.kind === "delete" ? "trash" : "copy"
                                size: 13; color: card.clash ? Theme.rose : Theme.muted
                            }
                            Text {
                                anchors { left: kindIcon.right; leftMargin: 8; right: parent.right; rightMargin: 26; verticalCenter: parent.verticalCenter }
                                text: card.clash ? "“" + card.c.dst.substring(card.c.dst.lastIndexOf("/") + 1) + "” is already in " + (card.c.dst.substring(0, card.c.dst.lastIndexOf("/")).split("/").pop() || "/")
                                    : card.ok ? card.job.label.replace(/^Copying/, "Copied").replace(/^Moving/, "Moved").replace(/^Deleting/, "Deleted").replace(/^Extracting/, "Extracted").replace(/^Compressing/, "Compressed")
                                    : card.job.label
                                elide: Text.ElideMiddle
                                color: Theme.text
                                font { family: Theme.font; pointSize: Theme.pt(12); weight: Font.Medium }
                            }
                            IconButton {
                                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                                width: 22; height: 22; iconSize: 11; fill: "transparent"
                                icon: "x"
                                onClicked: card.done ? Ops.dismiss(card.job.id) : Ops.cancel(card.job.id)
                            }
                        }

                        // ---- how far ----
                        Rectangle {
                            visible: !card.done && !card.clash
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
                                if (card.clash) return card.c.folders ? "Merging puts both together; files that are there and the same are skipped without asking."
                                    : "Coming " + F.size(card.c.srcSize) + " · " + F.date(card.c.srcModified, root.now) + "   ·   there " + F.size(card.c.dstSize) + " · " + F.date(card.c.dstModified, root.now);
                                if (card.done) {
                                    if (j.cancelled) return "Stopped";
                                    if (j.failed.length === 0) return "";
                                    const first = j.failed[0];
                                    const name = first[0].substring(first[0].lastIndexOf("/") + 1);
                                    return "“" + name + "”: " + first[1] + (j.failed.length > 1 ? ", and " + (j.failed.length - 1) + " more" : "");
                                }
                                if (j.bytesTotal > 0 && card.transfer) {
                                    const parts = [Math.round(card.fraction * 100) + "%", F.size(j.bytesDone) + " of " + F.size(j.bytesTotal)];
                                    if (j.rate > 0) parts.push(F.size(j.rate) + "/s", F.eta(j.bytesTotal - j.bytesDone, j.rate));
                                    return parts.filter(p => p).join(" · ");
                                }
                                return j.itemsDone + " of " + j.itemsTotal + " items";
                            }
                            wrapMode: Text.Wrap
                            color: card.done && card.job.failed.length > 0 ? Theme.rose : Theme.muted
                            font { family: Theme.font; pointSize: Theme.pt(10.5) }
                        }

                        // ---- a text clash: the changes, here ----
                        Rectangle {
                            visible: card.wide
                            width: parent.width; height: Math.min(130, diffList.contentHeight + 8)
                            radius: 8
                            color: Theme.bg
                            ListView {
                                id: diffList
                                anchors { fill: parent; margins: 4 }
                                clip: true
                                boundsBehavior: Flickable.StopAtBounds
                                ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                                model: card.wide ? card.diff.hunks.reduce((a, h, i) => a.concat(i > 0 ? [{ kind: "gap" }] : [], h), []) : []
                                delegate: Rectangle {
                                    id: line
                                    required property var modelData
                                    width: diffList.width
                                    height: 18
                                    color: modelData.kind === "+" ? root.addBg : modelData.kind === "-" ? root.delBg : "transparent"
                                    Text {
                                        x: 8; width: 12
                                        anchors.verticalCenter: parent.verticalCenter
                                        text: line.modelData.kind === "gap" ? "⋯" : line.modelData.kind === " " ? "" : line.modelData.kind
                                        color: line.modelData.kind === "+" ? root.addInk : line.modelData.kind === "-" ? Theme.rose : Theme.faint
                                        font { family: Theme.mono; pointSize: Theme.pt(10); weight: Font.DemiBold }
                                    }
                                    Text {
                                        x: 24; width: parent.width - 28
                                        anchors.verticalCenter: parent.verticalCenter
                                        text: line.modelData.text ?? ""
                                        textFormat: Text.PlainText
                                        elide: Text.ElideRight
                                        color: Theme.text
                                        font { family: Theme.mono; pointSize: Theme.pt(10) }
                                    }
                                }
                            }
                        }

                        // ---- what to do (a clash; transfers; failed trashing) ----
                        Item {
                            visible: card.clash
                            width: parent.width; height: 30
                            Row {
                                anchors.verticalCenter: parent.verticalCenter
                                spacing: 7
                                Toggle { anchors.verticalCenter: parent.verticalCenter; checked: card.forAll; onToggled: on => card.forAll = on }
                                Text { anchors.verticalCenter: parent.verticalCenter; text: "Same for the rest"; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(10.5) } }
                            }
                            Row {
                                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                                spacing: 4
                                Button { text: "Skip"; kind: "ghost"; onClicked: Ops.answer(card.job.id, "skip", card.forAll) }
                                Button { text: "Keep both"; onClicked: Ops.answer(card.job.id, "keep", card.forAll) }
                                // A folder: merge, replacing only older files inside.
                                Button {
                                    text: card.c.folders ? "Merge, keep newer" : "Keep newer"
                                    visible: card.c.folders || card.c.srcModified !== card.c.dstModified
                                    onClicked: Ops.answer(card.job.id, "newer", card.forAll || card.c.folders)
                                }
                                Button { text: card.c.folders ? "Merge" : "Replace"; kind: "primary"; onClicked: Ops.answer(card.job.id, "replace", card.forAll) }
                            }
                        }
                        Row {
                            visible: card.transfer && (card.job.state === "running" || card.job.state === "paused")
                            spacing: 6
                            Button { anchors.verticalCenter: undefined; text: card.job.state === "paused" ? "Carry on" : "Pause"; onClicked: Ops.pause(card.job.id, card.job.state !== "paused") }
                            Button { anchors.verticalCenter: undefined; text: "Cancel"; kind: "ghost"; onClicked: Ops.cancel(card.job.id) }
                        }
                        Button {
                            visible: card.done && card.job.kind === "trash" && card.job.failed.length > 0
                            text: "Delete permanently instead"
                            kind: "danger"
                            anchors.verticalCenter: undefined
                            onClicked: { root.deleteInstead(card.job.failed.map(f => f[0])); Ops.dismiss(card.job.id) }
                        }
                    }
                }
            }
        }
    }
}
