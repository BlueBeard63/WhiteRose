// New notifications, popped up where the notifications panel opens, newest on
// top (at most three). Each leaves after its timeout (paused while hovered);
// critical ones stay until dismissed. Nothing pops up while a panel is open.
import QtQuick
import "Notifications.js" as N

SidePanel {
    id: root
    open: Notifications.popups.length > 0 && ShellState.panel === ""
    takesKeyboard: false
    spacing: 10

    Repeater {
        model: Notifications.popups
        delegate: NotificationCard {
            required property var modelData
            notification: modelData
            timeout: N.popupTimeout(modelData.urgency, modelData.expireTimeout)
            onTimedOut: Notifications.hidePopup(modelData)
        }
    }
}
