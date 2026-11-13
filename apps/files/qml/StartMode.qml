// Start ("start:"): a page to begin from. Your workspaces (open one, or
// save what's open as one), your places, what you opened or changed
// lately, your drives and how full they are, and your tags. Sections with
// nothing in them aren't shown.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F

Flickable {
    id: start
    required property var pane
    contentHeight: col.implicitHeight + 48
    clip: true
    focus: true
    boundsBehavior: Flickable.StopAtBounds
    ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }

    // What's shown, looked at again when the page is (and every few seconds while it is).
    property var places: ({ folders: [], bookmarks: [], drives: [] })
    property var recent: []
    property var drives: []
    property var tagCounts: ({})
    property var workspaces: []
    function refresh() {
        start.places = JSON.parse(Fs.places());
        const r = JSON.parse(Fs.list("recent:", false, "modified", true, 48, "mixed"));
        start.recent = r.ok ? r.entries.slice(0, 8) : [];
        start.drives = JSON.parse(Fs.storage());
        start.tagCounts = JSON.parse(Fs.tagCounts());
        start.workspaces = JSON.parse(Fs.workspaces());
    }
    Component.onCompleted: refresh()
    Timer { interval: 5000; repeat: true; running: start.visible && Qt.application.state === Qt.ApplicationActive; onTriggered: start.refresh() }
    Connections { target: Session; function onWorkspacesChanged() { start.refresh() } }

    readonly property real inner: width - 48
    readonly property string greeting: {
        const h = new Date().getHours();
        return h < 5 ? "Good night" : h < 12 ? "Good morning" : h < 18 ? "Good afternoon" : "Good evening";
    }

    component Heading: Text {
        topPadding: 8
        color: Theme.faint
        font { family: Theme.font; pointSize: Theme.pt(10.5); weight: Font.Medium; letterSpacing: 0.8; capitalization: Font.AllUppercase }
    }
    // A tile: an icon, a name, a line under it.
    component Tile: Rectangle {
        id: tile
        property string icon
        property string name
        property string sub
        property real fill: -1          // a drive: how full (0..1)
        signal clicked()
        signal secondary()
        width: 200; height: tile.fill >= 0 ? 74 : 58
        radius: 12
        color: tArea.containsMouse ? Theme.surface2 : Theme.surface1
        Image {
            id: tIcon
            x: 12; y: 12
            width: 30; height: 30
            sourceSize: Qt.size(60, 60)
            source: tile.icon ? F.fileUrl(tile.icon) : ""
        }
        Column {
            anchors { left: tIcon.right; leftMargin: 10; right: parent.right; rightMargin: 12; top: parent.top; topMargin: 11 }
            spacing: 2
            Text { width: parent.width; text: tile.name; elide: Text.ElideMiddle; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(12.5); weight: Font.Medium } }
            Text { visible: tile.sub !== ""; width: parent.width; text: tile.sub; elide: Text.ElideMiddle; color: Theme.faint; font { family: Theme.font; pointSize: Theme.pt(10.5) } }
        }
        Rectangle {
            visible: tile.fill >= 0
            anchors { left: parent.left; right: parent.right; bottom: parent.bottom; margins: 12 }
            height: 4; radius: 2; color: Theme.bg
            Rectangle { width: parent.width * Math.min(1, tile.fill); height: 4; radius: 2; color: tile.fill > 0.9 ? Theme.rose : Theme.muted }
        }
        MouseArea {
            id: tArea
            anchors.fill: parent
            hoverEnabled: true
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            cursorShape: Qt.PointingHandCursor
            onClicked: m => m.button === Qt.RightButton ? tile.secondary() : tile.clicked()
        }
    }

    Column {
        id: col
        x: 24; y: 24
        width: start.inner
        spacing: 14

        Column {
            spacing: 2
            Text { text: start.greeting; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(22); weight: Font.DemiBold } }
            Text { text: Qt.formatDate(new Date(), "dddd d MMMM"); color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(12) } }
        }

        // ---- workspaces ---------------------------------------------------------------
        Item {
            width: parent.width; height: 26
            Heading { anchors.verticalCenter: parent.verticalCenter; text: "Workspaces" }
            Button {
                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                text: "Save what's open…"; icon: "plus"
                onClicked: start.Window.window.askWorkspace("")
            }
        }
        Text {
            visible: start.workspaces.length === 0
            width: parent.width
            wrapMode: Text.Wrap
            text: "Save the windows and tabs you have open (and how their folders are shown) to come back to them later."
            color: Theme.faint
            font { family: Theme.font; pointSize: Theme.pt(11.5) }
        }
        Flow {
            width: parent.width
            spacing: 10
            Repeater {
                model: start.workspaces
                delegate: Tile {
                    id: wsTile
                    required property var modelData
                    icon: Fs.icon("view-grid", 32) || Fs.icon("folder", 32)
                    name: modelData.name
                    sub: modelData.tabs + (modelData.tabs === 1 ? " tab" : " tabs") + (modelData.windows > 1 ? " in " + modelData.windows + " windows" : "") + " · " + F.date(modelData.saved * 1000, Date.now())
                    onClicked: start.Window.window.openWorkspace(modelData.name)
                    onSecondary: wsMenu.openAt(20, height - 4)
                    Menu {
                        id: wsMenu
                        items: [
                            { label: "Open", run: () => start.Window.window.openWorkspace(wsTile.modelData.name) },
                            { label: "Open in a new window", run: () => start.Window.window.openWorkspace(wsTile.modelData.name, true) },
                            { label: "Save what's open as this", run: () => Session.saveWorkspace(wsTile.modelData.name) },
                            { separator: true },
                            { label: "Rename…", run: () => start.Window.window.askWorkspace(wsTile.modelData.name) },
                            { label: "Delete", danger: true, run: () => { Fs.deleteWorkspace(wsTile.modelData.name); start.refresh() } },
                        ]
                    }
                }
            }
        }

        // ---- places ------------------------------------------------------------------
        Heading { text: "Places" }
        Flow {
            width: parent.width
            spacing: 10
            Repeater {
                model: start.places.folders.concat(start.places.bookmarks).filter(p => p.path.startsWith("/"))
                delegate: Tile {
                    required property var modelData
                    icon: modelData.icon
                    name: modelData.name
                    sub: modelData.path.replace(Fs.home(), "~")
                    onClicked: start.pane.go(modelData.path)
                }
            }
        }

        // ---- recent --------------------------------------------------------------------
        Heading { visible: Session.recentFiles && start.recent.length > 0; text: "Recent" }
        Flow {
            visible: Session.recentFiles && start.recent.length > 0
            width: parent.width
            spacing: 10
            Repeater {
                model: Session.recentFiles ? start.recent : []
                delegate: Tile {
                    required property var modelData
                    icon: Fs.typeIcon(modelData.mime, false, 32)
                    name: modelData.name
                    sub: F.date(modelData.modified, Date.now())
                    onClicked: start.pane.activate(modelData)
                }
            }
            Tile {
                icon: Fs.icon("document-open-recent", 32)
                name: "All recent files"
                sub: ""
                onClicked: start.pane.go("recent:")
            }
        }

        // ---- drives --------------------------------------------------------------------
        Heading { visible: start.drives.length > 0; text: "Drives" }
        Flow {
            width: parent.width
            spacing: 10
            Repeater {
                model: start.drives
                delegate: Tile {
                    required property var modelData
                    icon: modelData.icon
                    name: modelData.name
                    sub: F.size(modelData.free) + " free of " + F.size(modelData.total)
                    fill: modelData.total > 0 ? (modelData.total - modelData.free) / modelData.total : 0
                    onClicked: start.pane.go(modelData.path)
                }
            }
        }

        // ---- tags ------------------------------------------------------------------------
        Heading { visible: Object.keys(start.tagCounts).length > 0; text: "Tags" }
        Flow {
            width: parent.width
            spacing: 8
            Repeater {
                model: Object.keys(start.tagCounts)
                delegate: Rectangle {
                    required property string modelData
                    width: tagRow.implicitWidth + 24; height: 30; radius: 15
                    color: tagArea.containsMouse ? Theme.surface2 : Theme.surface1
                    Row {
                        id: tagRow
                        anchors.centerIn: parent
                        spacing: 8
                        Rectangle { anchors.verticalCenter: parent.verticalCenter; width: 9; height: 9; radius: 4.5; color: Session.tagColour(modelData) }
                        Text { text: modelData; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(12) } }
                        Text { text: start.tagCounts[modelData]; color: Theme.faint; font { family: Theme.font; pointSize: Theme.pt(11) } }
                    }
                    MouseArea { id: tagArea; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: start.pane.go("tag:" + modelData) }
                }
            }
        }
    }
}
