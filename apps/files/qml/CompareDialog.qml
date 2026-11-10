// Compare & sync (☰ › This folder › Compare with…, or with the other side
// when split): what differs between two folders, all the way down, and
// syncing them as a job (compare.rs). Sizes and times by default; byte by
// byte when asked. Syncing shows what it will do first; whatever a sync
// removes goes to the Trash.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F

Popup {
    id: root
    property string left: ""
    property string right: ""
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
    readonly property var plan: root.looked && !root.scanning ? JSON.parse(Ops.syncPlan(root.left, root.right, JSON.stringify(root.entries),
        root.how.startsWith("mirror") ? "mirror" : root.how === "two-way" ? "two-way" : "update", root.how.endsWith("left") ? "left" : "right")) : null
    readonly property int trashing: root.plan ? root.plan.trash_right.length + root.plan.trash_left.length : 0
    property bool confirming: false
    signal synced(int job)

    function name(p) { return p.substring(p.lastIndexOf("/") + 1) || p }
    function show_(left, right) {
        root.left = left;
        root.right = right;
        root.entries = [];
        root.looked = false;
        root.show = "all";
        root.confirming = false;
        root.open();
        if (left && right) root.look();
    }
    function look() {
        if (root.scanning) Ops.stopCleanup(root.scan);
        if (!root.left || !root.right || root.left === root.right) return;
        root.entries = [];
        root.scanned = 0;
        root.scanning = true;
        root.looked = true;
        root.confirming = false;
        root.scan = Ops.compare(root.left, root.right, root.content);
    }
    function swap() { const l = root.left; root.left = root.right; root.right = l; root.look() }
    onHowChanged: root.confirming = false
    onClosed: if (root.scanning) { Ops.stopCleanup(root.scan); root.scanning = false }
    Connections {
        target: Ops
        function onCompared(id, result) {
            if (id !== root.scan) return;
            const r = JSON.parse(result);
            if (!r.done) { root.scanned = r.scanned; return }
            root.scanning = false;
            root.entries = r.entries;
        }
    }
    function go() {
        if (!root.plan) return;
        if (root.trashing > 0 && !root.confirming) { root.confirming = true; return }
        const id = Ops.sync(JSON.stringify(root.plan));
        if (id >= 0) root.synced(id);
        root.close();
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    width: Math.min(900, (parent?.width ?? 900) - 40)
    height: Math.min(720, (parent?.height ?? 720) - 40)
    modal: true
    padding: 20
    background: Rectangle { color: Theme.surface1; radius: 16; border { width: 1; color: Theme.surface2 } }
    Overlay.modal: Rectangle { color: Qt.rgba(0, 0, 0, 0.35) }

    FolderDialog {
        id: picker
        property string side: "right"
        title: "Compare with which folder?"
        onAccepted: {
            const p = decodeURIComponent(String(selectedFolder).replace(/^file:\/\//, ""));
            if (side === "left") root.left = p; else root.right = p;
            root.look();
        }
    }

    // A folder's place at the top: its name, where it is, and Change….
    component Place: Rectangle {
        id: pl
        property string path
        property string side
        height: 52; radius: 10
        color: Theme.bg
        border { width: 1; color: Theme.surface2 }
        Column {
            anchors { left: parent.left; leftMargin: 12; right: change.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
            Text { width: parent.width; elide: Text.ElideMiddle; text: pl.path ? root.name(pl.path) : "Choose a folder"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(12.5); weight: Font.Medium } }
            Text { width: parent.width; elide: Text.ElideMiddle; visible: pl.path !== ""; text: pl.path.replace(Fs.home(), "~"); color: Theme.faint; font { family: Theme.font; pointSize: Theme.pt(10.5) } }
        }
        Button { id: change; anchors { right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter } text: "Change…"; onClicked: { picker.side = pl.side; picker.open() } }
    }

    contentItem: Column {
        spacing: 12
        Text { text: "Compare & sync"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold } }
        Row {
            width: parent.width
            spacing: 8
            Place { width: (parent.width - 44) / 2; path: root.left; side: "left" }
            IconButton { anchors.verticalCenter: parent.verticalCenter; width: 28; height: 28; icon: "restart"; iconSize: 13; onClicked: root.swap(); ToolTip.visible: hovered; ToolTip.text: "Swap sides" }
            Place { width: (parent.width - 44) / 2; path: root.right; side: "right" }
        }
        Row {
            spacing: 14
            Row {
                spacing: 6
                anchors.verticalCenter: parent.verticalCenter
                Toggle { checked: root.content; onToggled: on => { root.content = on; root.look() } anchors.verticalCenter: parent.verticalCenter }
                Text { anchors.verticalCenter: parent.verticalCenter; text: "Compare contents (slower: reads every file the same size)"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
            }
            Button {
                text: root.scanning ? "Stop" : "Look again"
                enabled: root.left !== "" && root.right !== ""
                onClicked: root.scanning ? (Ops.stopCleanup(root.scan), root.scanning = false) : root.look()
            }
        }
        Segmented {
            anchors.verticalCenter: undefined
            visible: root.looked && !root.scanning && root.entries.length > 0
            value: root.show
            options: [
                { label: "All " + root.entries.length, value: "all" },
                { label: "Only in " + root.name(root.left), value: "only_left" },
                { label: "Only in " + root.name(root.right), value: "only_right" },
                { label: "Newer on the left", value: "left_newer" },
                { label: "Newer on the right", value: "right_newer" },
                { label: "Different", value: "differs" },
            ]
            onPicked: v => root.show = v
        }

        // ---- what differs ----
        Rectangle {
            width: parent.width
            height: root.availableHeight - y - foot.height - 12
            radius: 10
            color: Theme.bg
            border { width: 1; color: Theme.surface2 }
            Text {
                visible: root.scanning || root.entries.length === 0
                anchors.centerIn: parent
                text: root.scanning ? "Looked at " + Math.round(root.scanned).toLocaleString(Qt.locale(), "f", 0) + " items…"
                    : !root.left || !root.right ? "Choose two folders"
                    : root.left === root.right ? "That's the same folder twice"
                    : root.looked ? "They're the same" : ""
                color: Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(12) }
            }
            ListView {
                id: list
                visible: !root.scanning
                anchors { fill: parent; margins: 6 }
                clip: true
                model: root.shown
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                delegate: Item {
                    id: row
                    required property var modelData
                    width: list.width - 8
                    height: 40
                    readonly property var e: modelData
                    function info(s) {
                        if (!s) return "—";
                        return s.dir ? "Folder · " + s.files + " files · " + F.size(s.size) : F.size(s.size) + " · " + F.date(s.modified * 1000, Date.now());
                    }
                    readonly property var mark: ({
                        only_left: ["Only on the left", Theme.text], only_right: ["Only on the right", Theme.text],
                        left_newer: ["Newer on the left", Theme.text], right_newer: ["Newer on the right", Theme.text],
                        differs: ["Different, same time", Theme.rose],
                    })[e.state]
                    Text {
                        x: 8; y: 4
                        width: parent.width - 16
                        elide: Text.ElideMiddle
                        text: row.e.rel
                        color: Theme.text
                        font { family: Theme.font; pointSize: Theme.pt(11.5); weight: Font.Medium }
                    }
                    Text { x: 8; y: 22; width: parent.width * 0.36; elide: Text.ElideRight; text: row.info(row.e.left); color: row.e.state === "left_newer" ? Theme.text : Theme.faint; font { family: Theme.font; pointSize: Theme.pt(10.5) } }
                    Text { x: parent.width * 0.38; y: 22; width: parent.width * 0.24; horizontalAlignment: Text.AlignHCenter; text: row.mark[0]; color: row.mark[1]; font { family: Theme.font; pointSize: Theme.pt(10.5); weight: Font.Medium } }
                    Text { x: parent.width * 0.64; y: 22; width: parent.width * 0.36 - 8; horizontalAlignment: Text.AlignRight; elide: Text.ElideRight; text: row.info(row.e.right); color: row.e.state === "right_newer" ? Theme.text : Theme.faint; font { family: Theme.font; pointSize: Theme.pt(10.5) } }
                    Rectangle { anchors.bottom: parent.bottom; x: 8; width: parent.width - 16; height: 1; color: Theme.surface1 }
                }
            }
        }

        // ---- syncing ----
        Item {
            id: foot
            width: parent.width; height: 56
            Column {
                anchors { left: parent.left; verticalCenter: parent.verticalCenter }
                width: parent.width - buttons.width - 16
                spacing: 4
                Choice {
                    anchors.verticalCenter: undefined
                    enabled: root.plan !== null && root.entries.length > 0
                    value: root.how
                    options: [
                        { label: "Update " + root.name(root.right) + ": new and newer from the left", value: "update-right" },
                        { label: "Update " + root.name(root.left) + ": new and newer from the right", value: "update-left" },
                        { label: "Two-way: each gets what's new or newer on the other", value: "two-way" },
                        { label: "Mirror to " + root.name(root.right) + ": make it exactly like the left", value: "mirror-right" },
                        { label: "Mirror to " + root.name(root.left) + ": make it exactly like the right", value: "mirror-left" },
                    ]
                    onPicked: v => root.how = v
                }
                Text {
                    width: parent.width
                    elide: Text.ElideRight
                    visible: root.plan !== null
                    readonly property var p: root.plan ?? { to_right: [], to_left: [], trash_right: [], trash_left: [], unsure: [] }
                    text: [p.to_right.length ? p.to_right.length + " copied to the right" : "", p.to_left.length ? p.to_left.length + " copied to the left" : "",
                           root.trashing ? root.trashing + " moved to the Trash" : "", p.unsure.length ? p.unsure.length + " left alone (can't tell which is newer)" : ""]
                        .filter(s => s).join(" · ") || "Nothing to do"
                    color: root.trashing ? Theme.rose : Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(11) }
                }
            }
            Row {
                id: buttons
                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                spacing: 8
                Button { text: "Close"; onClicked: root.close() }
                Button {
                    readonly property int n: root.plan ? root.plan.to_right.length + root.plan.to_left.length + root.trashing : 0
                    text: root.confirming ? "Sync and move " + root.trashing + " to the Trash" : "Sync"
                    kind: root.confirming ? "danger" : "primary"
                    enabled: n > 0 && !root.scanning
                    onClicked: root.go()
                }
            }
        }
    }
}
