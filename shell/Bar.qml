// The top bar (Figma: "Top Bar" component + idle island), one per screen.
// A Top-layer panel that reserves 26px; the island hangs below that and the
// input mask keeps clicks outside the visible shapes passing through.
import QtQuick
import Quickshell
import Quickshell.Wayland

PanelWindow {
    id: root
    required property ShellScreen modelData
    screen: modelData

    anchors { top: true; left: true; right: true }
    // Bar + whatever the island's current view needs (held while morphing
    // so the window never resizes mid-animation).
    implicitHeight: Theme.barHeight + Math.max(island.reserve, quick.reserve, notifications.reserve, popups.reserve) + 4
    exclusiveZone: Theme.barHeight
    color: "transparent"

    WlrLayershell.namespace: "whiterose-bar"
    WlrLayershell.layer: WlrLayer.Top
    // Open views take the keyboard (Esc closes; clicking elsewhere moves focus
    // away, which closes them too).
    WlrLayershell.keyboardFocus: island.wantsKeyboard || quick.open || notifications.open ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

    mask: Region {
        item: bar
        Region { item: island }
        Region { item: island.previewCard }
        Region { item: quick }
        Region { item: notifications }
        Region { item: popups }
    }

    Rectangle {
        id: bar
        anchors { top: parent.top; left: parent.left; right: parent.right }
        height: Theme.barHeight
        color: Theme.bg

        Row {
            id: left
            anchors { left: parent.left; leftMargin: 10; verticalCenter: parent.verticalCenter }
            spacing: 6
            Workspaces { anchors.verticalCenter: parent.verticalCenter }
            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: 1; height: 12
                color: Theme.surface2
                visible: media.visible
            }
            Media { id: media; anchors.verticalCenter: parent.verticalCenter }
        }

        Row {
            id: right
            anchors { right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
            spacing: 4
            Status {
                anchors.verticalCenter: parent.verticalCenter
                active: quick.open
                onClicked: ShellState.togglePanel("quick")
            }
            // The bell: a rose dot when something arrived since the panel was last open.
            Trigger {
                anchors.verticalCenter: parent.verticalCenter
                active: notifications.open
                onClicked: ShellState.togglePanel("notifications")
                Item {
                    anchors.verticalCenter: parent.verticalCenter
                    width: 14; height: 14
                    Icon { name: QuickState.doNotDisturb ? "bellOff" : "bell"; color: QuickState.doNotDisturb ? Theme.muted : Theme.text }
                    Rectangle {
                        visible: Notifications.unread && !QuickState.doNotDisturb
                        x: 9; y: -1
                        width: 6; height: 6; radius: 3
                        color: Theme.rose
                        border { width: 1; color: Theme.bg }
                    }
                }
            }
            Clock { anchors.verticalCenter: parent.verticalCenter }
        }
    }

    Island {
        id: island
        anchors { top: bar.bottom; horizontalCenter: parent.horizontalCenter }
    }

    QuickSettingsPanel {
        id: quick
        anchors { top: bar.bottom; right: parent.right }
    }
    NotificationsPanel {
        id: notifications
        anchors { top: bar.bottom; right: parent.right }
    }
    NotificationPopups {
        id: popups
        anchors { top: bar.bottom; right: parent.right }
    }

    Connections {
        target: Compositor
        function onAction(name) {
            if (["screenshot_region"].includes(name))
                console.info("whiterose:", name, "panel is not built yet");
        }
    }
}
