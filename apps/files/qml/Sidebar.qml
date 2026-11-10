// Places: your folders, bookmarks (shared with other apps' file choosers),
// and drives with how full they are. Right-click a bookmark to remove it.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F

Rectangle {
    id: side
    property string current: ""
    property var places: ({ folders: [], bookmarks: [], drives: [] })
    signal chosen(string path)
    // Files dropped on a place (the Trash trashes them).
    signal dropped(var paths, string dest)
    function refresh() {
        side.places = JSON.parse(Fs.places());
        side.counts = JSON.parse(Fs.tagCounts());
        side.loadKids();
    }
    // ---- the tree: folders opened in the sidebar, with their folders ------------------
    // {path: [{name, path, more}]} for each open folder (and whether a place has folders).
    property var kids: ({})
    // {path: true} for places with folders in them.
    property var hasKids: ({})
    function loadKids() {
        const k = {};
        for (const p of Session.sidebarOpen) k[p] = JSON.parse(Fs.subfolders(p, Session.hidden));
        side.kids = k;
        const h = {};
        for (const p of side.places.folders.concat(side.places.bookmarks))
            if (p.path.startsWith("/")) h[p.path] = Fs.hasSubfolders(p.path, Session.hidden);
        side.hasKids = h;
    }
    Connections { target: Session; function onSidebarOpenChanged() { side.loadKids() } }
    // A section's places with the folders opened under them, as one list:
    // [{name, path, icon, look, depth, more}].
    function withTree(list) {
        const out = [];
        const add = (path, depth) => {
            for (const c of side.kids[path] ?? []) {
                out.push({ name: c.name, path: c.path, icon: side.folderIcon, depth: depth, more: c.more });
                if (Session.sidebarOpen.includes(c.path)) add(c.path, depth + 1);
            }
        };
        for (const p of list) {
            out.push(Object.assign({ depth: 0, more: side.hasKids[p.path] === true }, p));
            if (Session.sidebarOpen.includes(p.path)) add(p.path, 1);
        }
        return out;
    }
    readonly property string folderIcon: Fs.icon("folder", 20)
    // How many files have each tag; the tags shown: Files' own, then any
    // found on files (from other apps).
    property var counts: ({})
    // A bookmark being dragged to a new place: which, and by how many rows.
    property string dragging: ""
    property int dragShift: 0
    readonly property var tagList: Session.tagDefs.map(t => t.name).concat(Object.keys(side.counts).filter(n => !Session.tagDefs.some(d => d.name === n)))
    // Right-click on a tag.
    signal tagMenu(string name, real x, real y)
    // A plugged-in drive clicked (not mounted yet), or its eject button.
    signal mountDrive(string device)
    signal ejectDrive(string device)
    Component.onCompleted: refresh()
    // Drives come and go (a USB stick): look again every few seconds.
    Timer { interval: 4000; repeat: true; running: Qt.application.state === Qt.ApplicationActive; onTriggered: side.refresh() }
    color: Theme.sidebar

    // A section's heading: click to fold the section away (remembered).
    component Heading: Item {
        id: hd
        property string text
        readonly property bool folded: Session.sidebarFolded.includes(hd.text)
        width: side.width; height: label.implicitHeight + 18
        Text {
            id: label
            x: 18; y: 14
            text: hd.text
            color: hdArea.containsMouse ? Theme.muted : Theme.faint
            font { family: Theme.font; pointSize: Theme.pt(10.5); weight: Font.Medium; letterSpacing: 0.8; capitalization: Font.AllUppercase }
        }
        Icon {
            anchors { right: parent.right; rightMargin: 16; verticalCenter: label.verticalCenter }
            visible: hdArea.containsMouse || hd.folded
            name: hd.folded ? "chevR" : "chevDown"
            size: 11
            color: Theme.faint
        }
        MouseArea { id: hdArea; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: Session.foldSidebar(hd.text) }
    }
    function open(section) { return !Session.sidebarFolded.includes(section) }
    component PlaceRow: Rectangle {
        id: pr
        property string name
        property string path
        property string icon
        property var look: null
        // In the tree: how deep, and whether it has folders to open (-1: not a tree row).
        property int depth: 0
        property bool more: false
        property bool tree: false
        readonly property bool open: Session.sidebarOpen.includes(pr.path)
        // A bookmark: dragged up or down to reorder (index in its list).
        property int order: -1
        signal reordered(int to)
        // A tag: its colour (a dot for an icon) and how many files have it.
        property color dot: "transparent"
        property int count: -1
        property real total: 0
        property real free: 0
        signal secondary()
        // Instead of going to `path` on a click (a drive to mount first).
        property var clickInstead: null
        readonly property bool on: side.current === path
        x: 8; width: side.width - 16
        height: total > 0 ? 44 : 32
        radius: 9
        color: on || prDrop.over ? Theme.surface1 : prArea.containsMouse ? Qt.rgba(Theme.surface1.r, Theme.surface1.g, Theme.surface1.b, 0.6) : "transparent"
        // Open or close the folders under it.
        Item {
            visible: pr.tree && pr.more
            x: 2 + pr.depth * 14; width: 16; height: pr.height
            Icon {
                anchors.centerIn: parent
                visible: prArea.containsMouse || pr.open || chevArea.containsMouse
                name: pr.open ? "chevDown" : "chevR"; size: 10
                color: chevArea.containsMouse ? Theme.text : Theme.faint
            }
            MouseArea { id: chevArea; anchors.fill: parent; hoverEnabled: true; z: 2; onClicked: Session.toggleSidebarOpen(pr.path) }
        }
        Image {
            id: prIcon
            x: (pr.tree ? 20 : 10) + pr.depth * 14; y: pr.total > 0 ? 8 : (pr.height - height) / 2
            width: 18; height: 18
            sourceSize: Qt.size(36, 36)
            // (An icon theme's file, or one of Files' own: qrc:.)
            source: !pr.icon ? "" : pr.icon.startsWith("qrc:") ? pr.icon : "file://" + pr.icon
            visible: !pr.look && pr.dot.a === 0
        }
        Rectangle {
            visible: pr.dot.a > 0
            anchors.centerIn: prIcon
            width: 11; height: 11; radius: 5.5
            color: pr.dot
        }
        FolderIcon { visible: !!pr.look; anchors.fill: prIcon; size: prIcon.width; look: pr.look ?? ({}) }
        Text {
            anchors { left: prIcon.right; leftMargin: 10; right: countText.left; rightMargin: 6 }
            y: pr.total > 0 ? 7 : (pr.height - height) / 2
            text: pr.name
            elide: Text.ElideRight
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(12.5); weight: pr.on ? Font.Medium : Font.Normal }
        }
        Text {
            id: countText
            anchors { right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
            visible: pr.count >= 0
            width: visible ? implicitWidth : 0
            text: pr.count
            color: Theme.faint
            font { family: Theme.font; pointSize: Theme.pt(11) }
        }
        // A drive: how full, and what's free.
        Rectangle {
            visible: pr.total > 0
            anchors { left: prIcon.right; leftMargin: 10; right: parent.right; rightMargin: 10 }
            y: 28; height: 4; radius: 2
            color: Theme.surface2
            Rectangle {
                width: parent.width * Math.min(1, (pr.total - pr.free) / Math.max(1, pr.total)); height: 4; radius: 2
                color: (pr.free / Math.max(1, pr.total)) < 0.1 ? Theme.rose : Theme.muted
            }
        }
        border { width: prDrop.over ? 1 : 0; color: Theme.tint(0.6) }
        FolderDrop { id: prDrop; anchors.fill: parent; dest: pr.path; onFilesDropped: (p, d) => side.dropped(p, d) }
        MouseArea {
            id: prArea
            anchors.fill: parent
            hoverEnabled: true
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            // Dragging a bookmark up or down moves it (rows of this height).
            property real startY: 0
            property bool moving: false
            onPressed: m => { startY = m.y; moving = false }
            onPositionChanged: m => {
                if (!pressed || pr.order < 0) return;
                if (!moving && Math.abs(m.y - startY) > 8) moving = true;
                if (moving) side.dragShift = Math.round((m.y - startY) / pr.height);
            }
            onReleased: {
                if (moving) { pr.reordered(Math.max(0, pr.order + side.dragShift)); side.dragShift = 0; side.dragging = "" }
            }
            onMovingChanged: if (moving) side.dragging = pr.path
            onClicked: m => { if (moving) return; m.button === Qt.RightButton ? pr.secondary() : pr.clickInstead ? pr.clickInstead() : side.chosen(pr.path) }
        }
        // Where a dragged bookmark would land.
        Rectangle {
            visible: side.dragging === pr.path && side.dragShift !== 0
            x: 6; width: parent.width - 12; height: 2; radius: 1
            y: side.dragShift > 0 ? pr.height * (side.dragShift + 1) - 1 : pr.height * side.dragShift - 1
            color: Theme.rose
        }
        ToolTip.visible: prArea.containsMouse && pr.total > 0
        ToolTip.delay: 600
        ToolTip.text: F.size(pr.free) + " free of " + F.size(pr.total)
    }

    Flickable {
        anchors.fill: parent
        contentHeight: col.implicitHeight + 16
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        Column {
            id: col
            width: parent.width
            spacing: 2
            Heading { text: "Places" }
            PlaceRow { visible: side.open("Places"); tree: true; name: "Start"; path: "start:"; icon: "qrc:/qt/qml/WhiteRose/Files/images/whiterose.png" }
            // Opened or changed lately (off with Settings › Privacy › Recent files).
            PlaceRow { visible: side.open("Places") && Session.recentFiles; tree: true; name: "Recent"; path: "recent:"; icon: Fs.icon("document-open-recent", 20) || Fs.icon("folder-recent", 20) }
            Repeater {
                model: side.open("Places") ? side.withTree(side.places.folders) : []
                delegate: PlaceRow { required property var modelData; tree: true; depth: modelData.depth; more: modelData.more; name: modelData.name; path: modelData.path; icon: modelData.icon; look: modelData.look ?? null }
            }
            Heading { visible: side.places.bookmarks.length > 0; text: "Bookmarks" }
            Repeater {
                model: side.open("Bookmarks") ? side.withTree(side.places.bookmarks) : []
                delegate: PlaceRow {
                    required property var modelData
                    tree: true; depth: modelData.depth; more: modelData.more
                    // Only the bookmarks themselves move (not folders opened under them).
                    order: modelData.depth === 0 ? side.places.bookmarks.findIndex(b => b.path === modelData.path) : -1
                    onReordered: to => { Fs.moveBookmark(modelData.path, to); side.refresh() }
                    name: modelData.name; path: modelData.path; icon: modelData.icon; look: modelData.look ?? null
                    onSecondary: bookmarkMenu.openAt(24, height - 2)
                    Menu {
                        id: bookmarkMenu
                        items: [{ label: "Remove bookmark", run: () => { Fs.setBookmark(modelData.path, false); side.refresh() } }]
                    }
                }
            }
            Heading { visible: side.tagList.length > 0; text: "Tags" }
            Repeater {
                model: side.open("Tags") ? side.tagList : []
                delegate: PlaceRow {
                    id: tagRow
                    required property string modelData
                    name: modelData
                    path: "tag:" + modelData
                    dot: Session.tagColour(modelData)
                    count: side.counts[modelData] ?? 0
                    onSecondary: { const p = tagRow.mapToItem(side, 24, tagRow.height - 2); side.tagMenu(modelData, p.x, p.y) }
                }
            }
            Heading { text: "Drives" }
            // Every drive at once, how full.
            PlaceRow { visible: side.open("Drives"); name: "Storage"; path: "drives:"; icon: Fs.icon("drive-multidisk", 20) || Fs.icon("drive-harddisk", 20) }
            Repeater {
                model: side.open("Drives") ? side.places.drives : []
                delegate: PlaceRow {
                    id: driveRow
                    required property var modelData
                    name: modelData.name; path: modelData.path; icon: modelData.icon; total: modelData.total; free: modelData.free
                    // Not mounted: dimmed; a click mounts it.
                    opacity: modelData.mounted ? 1 : 0.6
                    clickInstead: modelData.mounted ? null : () => side.mountDrive(driveRow.modelData.device)
                    IconButton {
                        visible: driveRow.modelData.ejectable && driveRow.modelData.mounted
                        anchors { right: parent.right; rightMargin: 4; top: parent.top; topMargin: 4 }
                        width: 24; height: 24; icon: "eject"; iconSize: 12; fill: "transparent"
                        onClicked: side.ejectDrive(driveRow.modelData.device)
                    }
                }
            }
        }
    }
}
