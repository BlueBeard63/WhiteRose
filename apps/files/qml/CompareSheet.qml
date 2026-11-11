// The Sheet's Compare and Sync tabs (☰ › This folder › Compare with…, or
// the other side when split): what differs between two folders, all the
// way down (compare.rs), marked on the files themselves in both panes (a
// coloured dot: only here, newer here, newer there, different, something
// inside differs) and listed here; then syncing them as a job. Sizes and
// times by default; byte by byte when asked. Syncing says what it will do
// first; whatever a sync removes goes to the Trash.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F

Column {
    id: root
    // compare | sync: which half is shown.
    property string part: "compare"
    // The panes of the tab (marked when they show one of the two folders).
    property var panes: []
    property string leftDir: ""
    property string rightDir: ""
    property bool content: false
    property int scan: -1
    property bool scanning: false
    property bool looked: false
    property real scanned: 0
    // [{rel, state, left, right}] from compare.rs.
    property var entries: []
    // all | only_left | only_right | left_newer | right_newer | differs
    property string show: "all"
    readonly property var shown: root.show === "all" ? root.entries : root.entries.filter(e => e.state === root.show)
    // How to sync: update-right | update-left | two-way | mirror-right | mirror-left
    property string how: "update-right"
    readonly property var plan: root.looked && !root.scanning ? JSON.parse(Ops.syncPlan(root.leftDir, root.rightDir, JSON.stringify(root.entries),
        root.how.startsWith("mirror") ? "mirror" : root.how === "two-way" ? "two-way" : "update", root.how.endsWith("left") ? "left" : "right")) : null
    readonly property int trashing: root.plan ? root.plan.trash_right.length + root.plan.trash_left.length : 0
    property bool confirming: false
    signal synced(int job)
    signal closed()

    readonly property var counts: {
        const c = { only_left: 0, only_right: 0, left_newer: 0, right_newer: 0, differs: 0 };
        for (const e of root.entries) c[e.state] = (c[e.state] ?? 0) + 1;
        return c;
    }
    readonly property var colours: ({ only_left: "#E8A3AE", only_right: "#539BF5", left_newer: "#57AB5A", right_newer: "#E3B341", differs: "#E5534B" })

    function name(p) { return p.substring(p.lastIndexOf("/") + 1) || p }
    function start(left, right) {
        root.leftDir = left;
        root.rightDir = right;
        root.entries = [];
        root.looked = false;
        root.show = "all";
        root.confirming = false;
        root.mark();
        if (left && right) root.look();
    }
    function look() {
        if (root.scanning) Ops.stopCleanup(root.scan);
        if (!root.leftDir || !root.rightDir || root.leftDir === root.rightDir) return;
        root.entries = [];
        root.scanned = 0;
        root.scanning = true;
        root.looked = true;
        root.confirming = false;
        root.mark();
        root.scan = Ops.compare(root.leftDir, root.rightDir, root.content);
    }
    function stop() {
        if (root.scanning) { Ops.stopCleanup(root.scan); root.scanning = false }
        root.entries = [];
        root.looked = false;
        root.mark();
    }
    function swap() { const l = root.leftDir; root.leftDir = root.rightDir; root.rightDir = l; root.look() }
    // The panes showing either folder: a mark on each thing that differs
    // (what's only, or newer, on that side; a folder whose insides differ).
    function mark() {
        // The same colours as the counts here; a folder with differences inside: grey.
        const words = { only_left: "Only in " + root.name(root.leftDir), only_right: "Only in " + root.name(root.rightDir),
                        left_newer: "Newer in " + root.name(root.leftDir), right_newer: "Newer in " + root.name(root.rightDir),
                        differs: "Different, same time" };
        const forSide = dir => {
            const m = {};
            for (const e of root.entries) {
                const top = e.rel.split("/")[0];
                const path = dir + "/" + top;
                if (e.rel.includes("/")) { if (!m[path]) m[path] = ["#8E8E96", "Something inside differs"] }
                else m[path] = [root.colours[e.state], words[e.state]];
            }
            return m;
        };
        for (const p of root.panes) {
            if (!p) continue;
            p.compared = root.entries.length === 0 ? {} : p.path === root.leftDir ? forSide(root.leftDir) : p.path === root.rightDir ? forSide(root.rightDir) : {};
        }
    }
    // (A pane going somewhere else: marked again for where it is now.)
    readonly property string paneKey: root.panes.map(p => p?.path ?? "").join("|")
    onPaneKeyChanged: mark()
    onHowChanged: root.confirming = false
    Connections {
        target: Ops
        function onCompared(id, result) {
            if (id !== root.scan) return;
            const r = JSON.parse(result);
            if (!r.done) { root.scanned = r.scanned; return }
            root.scanning = false;
            root.entries = r.entries;
            root.mark();
        }
    }
    function go() {
        if (!root.plan) return;
        if (root.trashing > 0 && !root.confirming) { root.confirming = true; return }
        const id = Ops.sync(JSON.stringify(root.plan));
        if (id >= 0) root.synced(id);
        root.stop();
        root.closed();
    }

    spacing: 12

    FolderDialog {
        id: picker
        property string side: "right"
        title: "Compare with which folder?"
        onAccepted: {
            const p = decodeURIComponent(String(selectedFolder).replace(/^file:\/\//, ""));
            if (side === "left") root.leftDir = p; else root.rightDir = p;
            root.look();
        }
    }

    // A folder, its place, and Change….
    component Place: Item {
        id: pl
        property string path
        property string side
        width: parent.width; height: 40
        Column {
            anchors { left: parent.left; right: change.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
            Text { width: parent.width; elide: Text.ElideMiddle; text: pl.path ? root.name(pl.path) : "Choose a folder"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(12.5); weight: Font.Medium } }
            Text { width: parent.width; elide: Text.ElideMiddle; visible: pl.path !== ""; text: pl.path.replace(Fs.home(), "~"); color: Theme.faint; font { family: Theme.font; pointSize: Theme.pt(10) } }
        }
        Text {
            id: change
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            text: "Change…"
            color: chMouse.containsMouse ? Theme.text : Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(11.5) }
            MouseArea { id: chMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: { picker.side = pl.side; picker.open() } }
        }
    }
    // A count with its colour (click: only those in the list below).
    component Count: Item {
        id: ct
        property string kind
        property string label
        readonly property int n: root.counts[ct.kind] ?? 0
        visible: n > 0
        width: parent.width; height: 24
        Rectangle { anchors.verticalCenter: parent.verticalCenter; width: 8; height: 8; radius: 4; color: root.colours[ct.kind] }
        Text {
            x: 16; anchors.verticalCenter: parent.verticalCenter
            text: ct.n + " " + ct.label
            color: root.show === ct.kind ? Theme.text : Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(12); weight: root.show === ct.kind ? Font.Medium : Font.Normal }
        }
        MouseArea { anchors.fill: parent; enabled: root.part === "compare"; cursorShape: Qt.PointingHandCursor; onClicked: root.show = root.show === ct.kind ? "all" : ct.kind }
    }

    Text {
        width: parent.width
        elide: Text.ElideMiddle
        text: root.part === "sync" ? "Sync " + root.name(root.leftDir) + " and " + root.name(root.rightDir) : "Compare two folders"
        color: Theme.text
        font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
    }

    // ---- the two folders ----
    Column {
        visible: root.part === "compare"
        width: parent.width
        Place { path: root.leftDir; side: "left" }
        Item {
            width: parent.width; height: 22
            Rectangle { anchors.verticalCenter: parent.verticalCenter; width: parent.width; height: 1; color: Theme.surface1 }
            Rectangle {
                anchors.centerIn: parent
                width: swapText.implicitWidth + 16; height: 22; radius: 11
                color: Theme.bg
                Text {
                    id: swapText
                    anchors.centerIn: parent
                    text: "⇅ swap"
                    color: swapMouse.containsMouse ? Theme.text : Theme.faint
                    font { family: Theme.font; pointSize: Theme.pt(10.5) }
                    MouseArea { id: swapMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: root.swap() }
                }
            }
        }
        Place { path: root.rightDir; side: "right" }
    }
    Item {
        visible: root.part === "compare"
        width: parent.width; height: 44
        Column {
            anchors.verticalCenter: parent.verticalCenter
            Text { text: "Compare contents"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(12) } }
            Text { text: "Reads files of the same size to be sure"; color: Theme.faint; font { family: Theme.font; pointSize: Theme.pt(10.5) } }
        }
        Toggle { anchors { right: parent.right; verticalCenter: parent.verticalCenter } checked: root.content; onToggled: on => { root.content = on; root.look() } }
    }

    // ---- what differs ----
    Text {
        visible: root.scanning || root.entries.length === 0
        width: parent.width
        wrapMode: Text.Wrap
        text: root.scanning ? "Looked at " + Math.round(root.scanned).toLocaleString(Qt.locale(), "f", 0) + " items…"
            : !root.leftDir || !root.rightDir ? "Choose two folders"
            : root.leftDir === root.rightDir ? "That's the same folder twice"
            : root.looked ? "They're the same" : ""
        color: Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(12) }
    }
    Column {
        visible: !root.scanning && root.entries.length > 0
        width: parent.width
        Count { kind: "only_left"; label: "only in " + root.name(root.leftDir) }
        Count { kind: "only_right"; label: "only in " + root.name(root.rightDir) }
        Count { kind: "left_newer"; label: "newer in " + root.name(root.leftDir) }
        Count { kind: "right_newer"; label: "newer in " + root.name(root.rightDir) }
        Count { kind: "differs"; label: "different, same time" }
    }
    Text {
        visible: root.part === "compare" && !root.scanning && root.entries.length > 0
        width: parent.width
        wrapMode: Text.Wrap
        text: "Marked on the files too, wherever either folder is open."
        color: Theme.faint
        font { family: Theme.font; pointSize: Theme.pt(10.5) }
    }
    // The differences, one a line (all, or the kind picked above).
    Column {
        visible: root.part === "compare" && !root.scanning && root.entries.length > 0
        width: parent.width
        Repeater {
            model: root.shown.slice(0, 200)
            delegate: Item {
                required property var modelData
                width: parent.width; height: 26
                Rectangle { anchors.verticalCenter: parent.verticalCenter; width: 6; height: 6; radius: 3; color: root.colours[modelData.state] }
                Text {
                    x: 14; width: parent.width - 14
                    anchors.verticalCenter: parent.verticalCenter
                    elide: Text.ElideMiddle
                    text: modelData.rel
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(11) }
                }
            }
        }
        Text {
            visible: root.shown.length > 200
            text: "and " + (root.shown.length - 200) + " more"
            color: Theme.faint
            font { family: Theme.font; pointSize: Theme.pt(10.5) }
        }
    }

    // ---- syncing ----
    Column {
        visible: root.part === "sync" && root.looked && !root.scanning && root.entries.length > 0
        width: parent.width
        spacing: 4
        Text { text: "HOW"; color: Theme.faint; font { family: Theme.font; pointSize: Theme.pt(10); weight: Font.Medium; letterSpacing: 0.6 } }
        Repeater {
            model: [
                { value: "update-right", title: "Update " + root.name(root.rightDir), sub: "new and newer from " + root.name(root.leftDir) },
                { value: "update-left", title: "Update " + root.name(root.leftDir), sub: "new and newer from " + root.name(root.rightDir) },
                { value: "two-way", title: "Two-way", sub: "each gets what's new or newer on the other" },
                { value: "mirror-right", title: "Mirror to " + root.name(root.rightDir), sub: "make it exactly like " + root.name(root.leftDir) },
                { value: "mirror-left", title: "Mirror to " + root.name(root.leftDir), sub: "make it exactly like " + root.name(root.rightDir) },
            ]
            delegate: Rectangle {
                required property var modelData
                readonly property bool on: root.how === modelData.value
                width: parent.width; height: 40; radius: 10
                color: on ? Theme.tint(0.14) : howMouse.containsMouse ? Theme.surface1 : "transparent"
                border { width: 1; color: on ? Theme.tint(0.6) : Theme.surface1 }
                Rectangle {
                    x: 12; anchors.verticalCenter: parent.verticalCenter
                    width: 14; height: 14; radius: 7
                    color: "transparent"
                    border { width: 1.5; color: parent.on ? Theme.rose : Theme.muted }
                    Rectangle { visible: parent.parent.on; anchors.centerIn: parent; width: 6; height: 6; radius: 3; color: Theme.rose }
                }
                Column {
                    x: 36; anchors.verticalCenter: parent.verticalCenter
                    width: parent.width - 44
                    Text { width: parent.width; elide: Text.ElideRight; text: modelData.title; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(12) } }
                    Text { width: parent.width; elide: Text.ElideRight; text: modelData.sub; color: Theme.faint; font { family: Theme.font; pointSize: Theme.pt(10.5) } }
                }
                MouseArea { id: howMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: root.how = modelData.value }
            }
        }
        // What it'll do (and, before anything goes to the Trash, asked again).
        Rectangle {
            width: parent.width
            height: planText.implicitHeight + 20
            radius: 10
            color: root.confirming ? Qt.rgba(0.9, 0.33, 0.29, 0.12) : Theme.surface1
            Text {
                id: planText
                x: 10; y: 10; width: parent.width - 20
                wrapMode: Text.Wrap
                readonly property var p: root.plan ?? { to_right: [], to_left: [], trash_right: [], trash_left: [], unsure: [] }
                text: ([p.to_right.length ? p.to_right.length + " copied to " + root.name(root.rightDir) : "", p.to_left.length ? p.to_left.length + " copied to " + root.name(root.leftDir) : "",
                        root.trashing ? root.trashing + " to the Trash" : "", p.unsure.length ? p.unsure.length + " left alone (can't tell which is newer)" : ""]
                    .filter(s => s).join(" · ") || "Nothing to do")
                    + (root.confirming ? " — Sync again to go ahead (you can bring them back from the Trash)." : "")
                color: root.trashing ? Theme.text : Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(11) }
            }
        }
    }
    Row {
        spacing: 8
        Button {
            anchors.verticalCenter: undefined
            text: root.scanning ? "Stop" : root.part === "compare" ? "Done" : "Look again"
            kind: "ghost"
            onClicked: root.scanning ? (Ops.stopCleanup(root.scan), root.scanning = false) : root.part === "compare" ? (root.stop(), root.closed()) : root.look()
        }
        Button {
            visible: root.part === "sync"
            anchors.verticalCenter: undefined
            readonly property int n: root.plan ? root.plan.to_right.length + root.plan.to_left.length + root.trashing : 0
            text: root.confirming ? "Sync, " + root.trashing + " to the Trash" : "Sync"
            kind: root.confirming ? "danger" : "primary"
            enabled: n > 0 && !root.scanning
            onClicked: root.go()
        }
    }
}
