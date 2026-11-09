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
    function refresh() { side.places = JSON.parse(Fs.places()) }
    Component.onCompleted: refresh()
    // Drives come and go (a USB stick): look again every few seconds.
    Timer { interval: 4000; repeat: true; running: Qt.application.state === Qt.ApplicationActive; onTriggered: side.refresh() }
    color: Theme.sidebar

    component Heading: Text {
        leftPadding: 18; topPadding: 14; bottomPadding: 4
        color: Theme.faint
        font { family: Theme.font; pointSize: Theme.pt(10.5); weight: Font.Medium; letterSpacing: 0.8; capitalization: Font.AllUppercase }
    }
    component PlaceRow: Rectangle {
        id: pr
        property string name
        property string path
        property string icon
        property real total: 0
        property real free: 0
        signal secondary()
        readonly property bool on: side.current === path
        x: 8; width: side.width - 16
        height: total > 0 ? 44 : 32
        radius: 9
        color: on ? Theme.surface1 : prArea.containsMouse ? Qt.rgba(Theme.surface1.r, Theme.surface1.g, Theme.surface1.b, 0.6) : "transparent"
        Image {
            id: prIcon
            x: 10; y: pr.total > 0 ? 8 : (pr.height - height) / 2
            width: 18; height: 18
            sourceSize: Qt.size(36, 36)
            source: pr.icon ? "file://" + pr.icon : ""
        }
        Text {
            anchors { left: prIcon.right; leftMargin: 10; right: parent.right; rightMargin: 10 }
            y: pr.total > 0 ? 7 : (pr.height - height) / 2
            text: pr.name
            elide: Text.ElideRight
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(12.5); weight: pr.on ? Font.Medium : Font.Normal }
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
        MouseArea {
            id: prArea
            anchors.fill: parent
            hoverEnabled: true
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            onClicked: m => m.button === Qt.RightButton ? pr.secondary() : side.chosen(pr.path)
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
            Repeater {
                model: side.places.folders
                delegate: PlaceRow { required property var modelData; name: modelData.name; path: modelData.path; icon: modelData.icon }
            }
            Heading { visible: side.places.bookmarks.length > 0; text: "Bookmarks" }
            Repeater {
                model: side.places.bookmarks
                delegate: PlaceRow {
                    required property var modelData
                    name: modelData.name; path: modelData.path; icon: modelData.icon
                    onSecondary: bookmarkMenu.popup()
                    Menu {
                        id: bookmarkMenu
                        items: [{ label: "Remove bookmark", run: () => { Fs.setBookmark(modelData.path, false); side.refresh() } }]
                    }
                }
            }
            Heading { text: "Drives" }
            Repeater {
                model: side.places.drives
                delegate: PlaceRow { required property var modelData; name: modelData.name; path: modelData.path; icon: modelData.icon; total: modelData.total; free: modelData.free }
            }
        }
    }
}
