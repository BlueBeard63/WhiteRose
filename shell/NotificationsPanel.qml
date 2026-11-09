// Notifications (Figma: "Desktop - Notifications"): hangs from the right end
// of the bar like quick settings. Opened from the bell or SUPER+N. Lists every
// notification not yet dismissed, newest first; Clear all; the bell-off button
// toggles Do Not Disturb.
import QtQuick
import Quickshell

SidePanel {
    id: root
    name: "notifications"
    spacing: 10

    // Header.
    Item {
        width: parent.width
        height: 30
        Row {
            anchors.verticalCenter: parent.verticalCenter
            spacing: 8
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "Notifications"
                color: Theme.text
                font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
            }
            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                visible: Notifications.count > 0
                width: Math.max(22, countText.implicitWidth + 12); height: 18; radius: 9
                color: Theme.rose
                Text {
                    id: countText
                    anchors.centerIn: parent
                    text: Notifications.count
                    color: Theme.onAccent
                    font { family: Theme.font; pointSize: Theme.pt(11); weight: Font.DemiBold }
                }
            }
        }
        Row {
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            spacing: 6
            Text {
                anchors.verticalCenter: parent.verticalCenter
                visible: Notifications.count > 0
                text: "Clear all"
                color: clearMouse.containsMouse ? Theme.text : Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(12) }
                MouseArea {
                    id: clearMouse
                    anchors.fill: parent
                    anchors.margins: -6
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: Notifications.clearAll()
                }
            }
            IconButton {
                width: 28; height: 28; icon: "bellOff"; iconSize: 14
                fill: QuickState.doNotDisturb ? Theme.rose : "transparent"
                hoverFill: QuickState.doNotDisturb ? Theme.rose : Theme.surface2
                iconColor: QuickState.doNotDisturb ? Theme.onAccent : Theme.muted
                onClicked: QuickState.setDoNotDisturb(!QuickState.doNotDisturb)
            }
        }
    }

    // The list; scrolls once it's taller than about two thirds of the screen.
    ListView {
        id: list
        width: parent.width
        height: Notifications.count === 0 ? 0 : Math.min(contentHeight, (root.Window.window?.screen?.height ?? 800) * 0.66)
        visible: Notifications.count > 0
        clip: true
        spacing: 10
        boundsBehavior: Flickable.StopAtBounds
        model: Notifications.list
        delegate: NotificationCard { required property var modelData; notification: modelData }
        add: Transition { NumberAnimation { property: "opacity"; from: 0; to: 1; duration: 160 } }
        displaced: Transition { NumberAnimation { property: "y"; duration: 180; easing.type: Easing.OutCubic } }
    }

    Text {
        visible: Notifications.count === 0
        width: parent.width
        height: 48
        horizontalAlignment: Text.AlignHCenter
        verticalAlignment: Text.AlignVCenter
        text: QuickState.doNotDisturb ? "Do Not Disturb is on" : "No notifications"
        color: Theme.faint
        font { family: Theme.font; pointSize: Theme.pt(13) }
    }
}
