// One app in the dock (Figma: Dock/Tile): the app's real icon on a dark tile,
// a dot while it runs, a wide pearl dot when focused.
import QtQuick
import Quickshell

Item {
    id: root
    // { key, entry, appId, running, focused, pinned }
    required property var app
    signal activated()
    signal newWindow()
    signal togglePin()

    readonly property bool hovered: mouse.containsMouse

    width: 42
    height: 42 + 5 + 4

    Rectangle {
        id: tile
        width: 42; height: 42; radius: 13
        color: root.app.focused ? Theme.surface2 : mouse.containsMouse ? Qt.lighter(Theme.surface1, 1.35) : Theme.surface1
        Behavior on color { ColorAnimation { duration: Theme.fastAnim } }
        scale: mouse.pressed ? 0.92 : 1
        Behavior on scale { NumberAnimation { duration: 90 } }

        Image {
            id: icon
            anchors.centerIn: parent
            width: 26; height: 26
            source: root.app.entry ? Quickshell.iconPath(root.app.entry.icon, true) : ""
            sourceSize: Qt.size(52, 52)   // crisp at 2x, small in memory
            asynchronous: true
            visible: status === Image.Ready
        }
        // No icon in the theme: the app's initial instead of a broken image.
        Text {
            anchors.centerIn: parent
            visible: icon.status !== Image.Ready
            text: (root.app.entry?.name ?? root.app.appId ?? "?").charAt(0).toUpperCase()
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(16); weight: Font.DemiBold }
        }
    }

    Rectangle {
        anchors { top: tile.bottom; topMargin: 5; horizontalCenter: tile.horizontalCenter }
        visible: root.app.running
        width: root.app.focused ? 10 : 4
        height: 4
        radius: 2
        color: root.app.focused ? Theme.pearl : Theme.muted
        Behavior on width { NumberAnimation { duration: Theme.fastAnim } }
    }

    MouseArea {
        id: mouse
        anchors.fill: tile
        hoverEnabled: true
        acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton
        cursorShape: Qt.PointingHandCursor
        onClicked: event => {
            if (event.button === Qt.RightButton) root.togglePin();
            else if (event.button === Qt.MiddleButton) root.newWindow();
            else root.activated();
        }
    }
}
