// One notification (Figma: the cards in "Desktop - Notifications"): app icon,
// app name, age, title, body and the app's action buttons. Clicking it runs
// its default action (or brings the app forward); the × on hover dismisses.
import QtQuick
import Quickshell
import Quickshell.Widgets
import "Notifications.js" as N

Rectangle {
    id: root
    required property var notification
    readonly property var n: notification
    // Popups get a countdown (ms; 0 = stays) that pauses while hovered.
    property int timeout: 0
    signal timedOut()

    width: 324
    implicitHeight: body.implicitHeight + 24
    radius: 16
    color: mouse.containsMouse ? Theme.surface2 : Theme.surface1
    Behavior on color { ColorAnimation { duration: Theme.fastAnim } }

    // The age label refreshes once a minute.
    property real now: Date.now()
    Timer { interval: 60000; running: true; repeat: true; onTriggered: root.now = Date.now() }

    Timer {
        running: root.timeout > 0 && !mouse.containsMouse && !closeBtn.hovered
        interval: root.timeout
        onTriggered: root.timedOut()
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: { Notifications.activate(root.n); ShellState.closePanel() }
    }

    // App icon: the notification's image (e.g. a sender's picture), else the
    // app's icon, else a bell.
    ClippingRectangle {
        id: icon
        x: 12; y: 12
        width: 30; height: 30; radius: 9
        color: Theme.surface2
        readonly property var kind: N.iconKind(root.n.appIcon)
        // The notification's own image, unless it's a theme icon the theme lacks.
        readonly property string themed: N.themedImageName(root.n.image)
        readonly property string image: themed !== "" && !Quickshell.hasThemeIcon(themed) ? "" : root.n.image
        readonly property string source:
            image !== "" ? image
            : kind.kind === "url" ? kind.value
            : kind.kind === "name" ? Quickshell.iconPath(kind.value, true)
            : ""
        Image {
            id: pic
            anchors.fill: parent
            // Pictures (a sender's photo) fill the tile; icons get a margin.
            anchors.margins: icon.image !== "" && icon.themed === "" ? 0 : 5
            source: icon.source
            sourceSize: Qt.size(60, 60)
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            visible: status === Image.Ready
        }
        Icon { anchors.centerIn: parent; visible: pic.status !== Image.Ready; name: "bell"; size: 15; color: Theme.pearl }
    }

    Column {
        id: body
        x: icon.x + icon.width + 10
        y: 12
        width: root.width - x - 12
        spacing: 2

        Item {
            width: parent.width
            height: appName.implicitHeight
            Text {
                id: appName
                width: parent.width - 30
                text: root.n.appName || "Notification"
                elide: Text.ElideRight
                color: Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(11) }
            }
            Text {
                anchors.right: parent.right
                visible: !mouse.containsMouse && !closeBtn.hovered
                text: N.ago((root.now - Notifications.arrived(root.n)) / 1000)
                color: Theme.faint
                font { family: Theme.font; pointSize: Theme.pt(11) }
            }
        }
        Text {
            width: parent.width
            text: root.n.summary
            wrapMode: Text.Wrap
            maximumLineCount: 2
            elide: Text.ElideRight
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.Medium }
        }
        Text {
            width: parent.width
            visible: text !== ""
            text: root.n.body
            textFormat: Text.PlainText
            wrapMode: Text.Wrap
            maximumLineCount: 3
            elide: Text.ElideRight
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(12) }
        }
        // The app's buttons.
        Flow {
            width: parent.width
            visible: N.buttons(root.n.actions).length > 0
            topPadding: 8
            spacing: 6
            Repeater {
                model: N.buttons(root.n.actions)
                delegate: Rectangle {
                    required property var modelData
                    width: label.implicitWidth + 20
                    height: 26
                    radius: 8
                    color: btnMouse.containsMouse ? Theme.grip : Theme.surface2
                    Text {
                        id: label
                        anchors.centerIn: parent
                        text: modelData.text
                        color: Theme.text
                        font { family: Theme.font; pointSize: Theme.pt(12); weight: Font.Medium }
                    }
                    MouseArea {
                        id: btnMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: modelData.invoke()
                    }
                }
            }
        }
    }

    IconButton {
        id: closeBtn
        anchors { right: parent.right; top: parent.top; margins: 8 }
        visible: mouse.containsMouse || hovered
        width: 22; height: 22
        icon: "x"; iconSize: 11
        fill: Theme.surface2; hoverFill: Theme.rose
        iconColor: hovered ? Theme.bg : Theme.text
        onClicked: Notifications.dismiss(root.n)
    }
}
