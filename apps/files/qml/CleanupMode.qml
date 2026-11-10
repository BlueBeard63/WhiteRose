// Clean up ("cleanup:"): what takes room under a folder (home, a drive, or
// the folder you came from), to move to the Trash or delete. Three looks,
// each started when asked (they read a lot): the largest files, duplicates
// (same contents), and build caches and downloaded dependencies (made
// again when needed). Largest and duplicates go to the Trash by default:
// nothing's gone until it's emptied.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F

Item {
    id: page
    required property var pane

    // largest | duplicates | caches
    property string kind: "largest"
    property string root: Session.cleanupRoot || Fs.home()
    // Where it can look: home, each drive, and the folder it was asked from.
    readonly property var roots: {
        const home = Fs.home();
        const v = [{ label: "Home", value: home }];
        for (const d of JSON.parse(Fs.storage())) if (d.path !== home) v.push({ label: d.name, value: d.path });
        const from = Session.cleanupRoot;
        if (from && !v.some(o => o.value === from)) v.push({ label: from.replace(home, "~"), value: from });
        return v;
    }
    property int scan: -1
    property real scanned: 0
    property bool scanning: false
    property bool looked: false
    // [{path, size, modified, what, group?, first?, count?}]: one list (duplicates: by group).
    property var rows: []
    property var picked: ({})
    readonly property var chosen: page.rows.filter(r => page.picked[r.path])
    readonly property real chosenBytes: page.chosen.reduce((a, r) => a + r.size, 0)

    function look() {
        if (page.scanning) Ops.stopCleanup(page.scan);
        page.rows = [];
        page.picked = {};
        page.scanned = 0;
        page.scanning = true;
        page.looked = true;
        page.scan = Ops.cleanup(page.kind, page.root);
    }
    function stop() { Ops.stopCleanup(page.scan); page.scanning = false }
    onKindChanged: { if (page.scanning) page.stop(); page.rows = []; page.picked = {}; page.looked = false }
    onRootChanged: { if (page.scanning) page.stop(); page.rows = []; page.picked = {}; page.looked = false }
    Component.onDestruction: if (page.scanning) Ops.stopCleanup(page.scan)

    Connections {
        target: Ops
        function onCleanupFound(id, result) {
            if (id !== page.scan) return;
            const r = JSON.parse(result);
            if (!r.done) { page.scanned = r.scanned; return }
            page.scanning = false;
            if (r.groups) {
                const rows = [];
                r.groups.forEach((g, gi) => g.files.forEach((f, i) => rows.push(Object.assign(f, { group: gi, first: i === 0, count: g.files.length }))));
                page.rows = rows;
            } else {
                page.rows = r.files;
            }
        }
    }
    function toggle(path) { const p = Object.assign({}, page.picked); if (p[path]) delete p[path]; else p[path] = true; page.picked = p }
    // Duplicates: everything but the oldest copy in each group.
    function pickAllButOldest() { const p = {}; for (const r of page.rows) if (!r.first) p[r.path] = true; page.picked = p }
    // Gone (trashed or deleted): off the list.
    function drop(paths) {
        const gone = new Set(paths);
        let rows = page.rows.filter(r => !gone.has(r.path));
        if (page.kind === "duplicates") {
            // A group down to one file isn't duplicates any more.
            const left = {};
            for (const r of rows) left[r.group] = (left[r.group] ?? 0) + 1;
            rows = rows.filter(r => left[r.group] > 1);
        }
        page.rows = rows;
        page.picked = {};
    }
    function act(how) {
        const paths = page.chosen.map(r => r.path);
        if (paths.length === 0) return;
        if (how === "trash") page.Window.window.run("trash", paths);
        else page.Window.window.askDelete(paths);
        if (how === "trash") page.drop(paths);
    }
    // Deleted after asking: off the list too.
    Connections {
        target: Ops
        function onFinished(id, result) {
            const r = JSON.parse(result);
            if (r.kind === "delete" && r.done.length > 0) page.drop(r.done);
        }
    }

    // ---- what to look at, and for what --------------------------------------------------------
    Column {
        id: head
        x: 24; y: 20
        width: page.width - 48
        spacing: 12
        Row {
            spacing: 10
            Text { anchors.verticalCenter: parent.verticalCenter; text: "Clean up"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(20); weight: Font.DemiBold } }
            Text { anchors.verticalCenter: parent.verticalCenter; text: "in"; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(13) } }
            Choice {
                value: page.root
                options: page.roots
                onPicked: v => page.root = v
            }
        }
        Row {
            spacing: 10
            Segmented {
                value: page.kind
                options: [{ label: "Largest files", value: "largest" }, { label: "Duplicates", value: "duplicates" }, { label: "Build caches", value: "caches" }]
                onPicked: v => page.kind = v
            }
            Button {
                text: page.scanning ? "Stop" : page.looked ? "Look again" : "Look"
                kind: page.scanning ? "plain" : "primary"
                onClicked: page.scanning ? page.stop() : page.look()
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                width: head.width - x
                elide: Text.ElideRight
                text: page.scanning ? "Looked at " + Math.round(page.scanned).toLocaleString(Qt.locale(), "f", 0) + " items…"
                    : page.looked ? (page.rows.length === 0 ? "Nothing found" : page.kind === "duplicates"
                        ? (page.rows.filter(r => r.first).length + " sets of copies · " + F.size(page.rows.filter(r => !r.first).reduce((a, r) => a + r.size, 0)) + " in extra copies")
                        : page.rows.length + " found · " + F.size(page.rows.reduce((a, r) => a + r.size, 0)))
                    : ({ largest: "The 200 largest files, hidden ones too",
                         duplicates: "Files of 1 MB or more with the same contents",
                         caches: "Build output and downloaded dependencies: made again when needed" })[page.kind]
                color: Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(11.5) }
            }
        }
    }

    // ---- what was found -------------------------------------------------------------------------
    ListView {
        id: list
        anchors { left: parent.left; right: parent.right; top: head.bottom; topMargin: 14; bottom: bar.top }
        clip: true
        model: page.rows
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
        delegate: Column {
            id: row
            required property var modelData
            required property int index
            width: list.width
            // Duplicates: a heading per set of copies.
            Item {
                visible: page.kind === "duplicates" && row.modelData.first
                width: parent.width; height: visible ? 34 : 0
                Text {
                    x: 24; anchors.bottom: parent.bottom; anchors.bottomMargin: 6
                    text: row.modelData.count + " copies · " + F.size(row.modelData.size) + " each"
                    color: Theme.faint
                    font { family: Theme.font; pointSize: Theme.pt(10.5); weight: Font.Medium; capitalization: Font.AllUppercase; letterSpacing: 0.6 }
                }
            }
            Rectangle {
                width: parent.width; height: 44
                color: page.picked[row.modelData.path] ? Theme.tint(0.12) : rArea.containsMouse ? Theme.surface1 : "transparent"
                Rectangle {
                    id: box
                    x: 24; anchors.verticalCenter: parent.verticalCenter
                    width: 18; height: 18; radius: 5
                    color: page.picked[row.modelData.path] ? Theme.rose : "transparent"
                    border { width: page.picked[row.modelData.path] ? 0 : 1.5; color: Theme.muted }
                    Icon { anchors.centerIn: parent; visible: !!page.picked[row.modelData.path]; name: "check"; size: 12; color: Theme.bg }
                }
                Image {
                    id: ic
                    anchors { left: box.right; leftMargin: 12; verticalCenter: parent.verticalCenter }
                    width: 24; height: 24
                    sourceSize: Qt.size(48, 48)
                    source: row.modelData.icon ? "file://" + row.modelData.icon : ""
                }
                Column {
                    anchors { left: ic.right; leftMargin: 10; right: sizeText.left; rightMargin: 16; verticalCenter: parent.verticalCenter }
                    spacing: 1
                    Text {
                        width: parent.width
                        text: row.modelData.path.substring(row.modelData.path.lastIndexOf("/") + 1) + (row.modelData.what ? "  ·  " + row.modelData.what : "")
                        elide: Text.ElideMiddle
                        color: Theme.text
                        font { family: Theme.font; pointSize: Theme.pt(12) }
                    }
                    Text {
                        width: parent.width
                        text: row.modelData.path.substring(0, row.modelData.path.lastIndexOf("/")).replace(Fs.home(), "~") + "  ·  " + F.date(row.modelData.modified, Date.now())
                            + (page.kind === "duplicates" && row.modelData.first ? "  ·  oldest" : "")
                        elide: Text.ElideMiddle
                        color: Theme.faint
                        font { family: Theme.font; pointSize: Theme.pt(10.5) }
                    }
                }
                Text {
                    id: sizeText
                    anchors { right: parent.right; rightMargin: 24; verticalCenter: parent.verticalCenter }
                    text: F.size(row.modelData.size)
                    color: Theme.muted
                    font { family: Theme.mono; pointSize: Theme.pt(11) }
                }
                MouseArea {
                    id: rArea
                    anchors.fill: parent
                    hoverEnabled: true
                    acceptedButtons: Qt.LeftButton
                    onClicked: page.toggle(row.modelData.path)
                    onDoubleClicked: page.Window.window.showInFolder({ path: row.modelData.path, name: row.modelData.path.substring(row.modelData.path.lastIndexOf("/") + 1) })
                }
            }
        }
    }

    // ---- what to do with what's ticked ------------------------------------------------------------
    Rectangle {
        id: bar
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
        height: page.rows.length > 0 ? 54 : 0
        visible: height > 0
        color: Theme.bg
        Rectangle { width: parent.width; height: 1; color: Theme.surface1 }
        Row {
            anchors { left: parent.left; leftMargin: 24; verticalCenter: parent.verticalCenter }
            spacing: 10
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: page.chosen.length === 0 ? "Tick what to remove" : page.chosen.length + " ticked · " + F.size(page.chosenBytes)
                color: Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(11.5) }
            }
            Button { visible: page.kind === "duplicates"; text: "Keep the oldest of each"; onClicked: page.pickAllButOldest() }
            Button { visible: page.chosen.length > 0; text: "Untick all"; kind: "ghost"; onClicked: page.picked = {} }
        }
        Row {
            anchors { right: parent.right; rightMargin: 24; verticalCenter: parent.verticalCenter }
            spacing: 8
            Button {
                visible: page.chosen.length === 1
                text: "Show in folder"
                onClicked: { const r = page.chosen[0]; page.Window.window.showInFolder({ path: r.path, name: r.path.substring(r.path.lastIndexOf("/") + 1) }) }
            }
            // Caches are made again when needed: trashing them frees nothing.
            Button {
                visible: page.kind !== "caches"
                text: "Move to Trash"; kind: "primary"
                enabled: page.chosen.length > 0
                onClicked: page.act("trash")
            }
            Button {
                text: "Delete…"; kind: page.kind === "caches" ? "primary" : "danger"
                enabled: page.chosen.length > 0
                onClicked: page.act("delete")
            }
        }
    }
}
