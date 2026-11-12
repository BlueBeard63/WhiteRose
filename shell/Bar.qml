// The top bar (Figma: "Top Bar" component + idle island), one per screen.
// A Top-layer panel that reserves 26px; the island hangs below that and the
// input mask keeps clicks outside the visible shapes passing through.
// With a fullscreen window on its screen the bar isn't drawn, but the
// island still is: this moves to the Overlay layer (above the window), the
// strip hides, and the island waits in the top edge until a view opens.
import QtQuick
import Quickshell
import Quickshell.Wayland

PanelWindow {
    id: root
    required property ShellScreen modelData
    screen: modelData
    readonly property bool fullscreen: Compositor.fullscreen.includes(modelData?.name ?? "")

    anchors { top: true; left: true; right: true }
    // Bar + whatever the island's current view needs (held while morphing
    // so the window never resizes mid-animation).
    implicitHeight: Theme.barHeight + Math.max(island.reserve, quick.reserve, notifications.reserve, calendar.reserve, popups.reserve) + 4
    exclusiveZone: Theme.barHeight
    color: "transparent"

    WlrLayershell.namespace: "whiterose-bar"
    LayerTitle { name: "whiterose-bar" }
    WlrLayershell.layer: root.fullscreen ? WlrLayer.Overlay : WlrLayer.Top
    // Open views take the keyboard (Esc closes; clicking elsewhere moves focus
    // away, which closes them too).
    WlrLayershell.keyboardFocus: island.wantsKeyboard || quick.open || notifications.open || calendar.open ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

    mask: Region {
        // (Fullscreen: no strip to click, only what the island shows.)
        item: root.fullscreen ? island : bar
        Region { item: island }
        // (Things a view puts outside the island: the dock's previews.)
        Region { item: island.inputs[0] ?? null }
        Region { item: island.inputs[1] ?? null }
        Region { item: quick }
        Region { item: notifications }
        Region { item: calendar }
        Region { item: popups }
    }

    Rectangle {
        id: bar
        anchors { top: parent.top; left: parent.left; right: parent.right }
        height: Theme.barHeight
        color: Theme.bg
        visible: !root.fullscreen

        Row {
            id: left
            anchors { left: parent.left; leftMargin: 10; verticalCenter: parent.verticalCenter }
            spacing: 6
            Workspaces { id: workspaces; anchors.verticalCenter: parent.verticalCenter; visible: Daemon.setting("bar.workspaces", true) }
            // This screen's window layout; a click moves to the next (SUPER+T).
            Trigger {
                id: layoutButton
                readonly property string layout: Compositor.windowLayouts[root.modelData?.name ?? ""] ?? "floating"
                label: "Layout"
                shows: layout.charAt(0).toUpperCase() + layout.slice(1)
                anchors.verticalCenter: parent.verticalCenter
                visible: workspaces.visible
                implicitWidth: 24
                onClicked: Compositor.nextWindowLayout(root.modelData?.name ?? "")
                Glyph {
                    y: Math.round((parent.height - height) / 2)
                    color: layoutButton.layout === "floating" ? Theme.muted : Theme.text
                    name: ({ dwindle: "layoutDwindle", master: "layoutMaster" })[layoutButton.layout] ?? "layoutFloating"
                }
            }
            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: 1; height: 12
                color: Theme.surface2
                visible: media.visible && workspaces.visible
            }
            Media { id: media; label: "Now playing"; anchors.verticalCenter: parent.verticalCenter; enabled: Daemon.setting("bar.media", true) }
        }

        Row {
            id: right
            anchors { right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
            spacing: 4
            Status {
                label: "Quick settings"
                anchors.verticalCenter: parent.verticalCenter
                active: quick.open
                onClicked: ShellState.togglePanel("quick")
            }
            // The bell: a rose dot when something arrived since the panel was last open.
            Trigger {
                label: "Notifications"
                shows: QuickState.doNotDisturb ? "Do Not Disturb" : Notifications.unread ? "unread" : ""
                anchors.verticalCenter: parent.verticalCenter
                visible: Daemon.setting("bar.bell", true)
                active: notifications.open
                onClicked: ShellState.togglePanel("notifications")
                Item {
                    y: Math.round((parent.height - height) / 2)
                    width: 16; height: 16
                    // Do Not Disturb: the moon.
                    Glyph { name: QuickState.doNotDisturb ? "moon" : "bell"; color: QuickState.doNotDisturb ? Theme.muted : Theme.text }
                    Rectangle {
                        visible: Notifications.unread && !QuickState.doNotDisturb
                        x: 10; y: 0
                        width: 6; height: 6; radius: 3
                        color: Theme.rose
                        border { width: 1; color: Theme.bg }
                    }
                }
            }
            Clock {
                label: "Clock"
                anchors.verticalCenter: parent.verticalCenter
                active: calendar.open
                onClicked: ShellState.togglePanel("calendar")
            }
        }
    }

    Island {
        id: island
        screenName: root.modelData?.name ?? ""
        fullscreen: root.fullscreen
        // Fullscreen: it hangs from the screen's top edge instead of the bar.
        anchors { top: bar.bottom; topMargin: root.fullscreen ? -Theme.barHeight : 0; horizontalCenter: parent.horizontalCenter }
    }

    QuickSettingsPanel {
        id: quick
        anchors { top: bar.bottom; right: parent.right }
    }
    NotificationsPanel {
        id: notifications
        anchors { top: bar.bottom; right: parent.right }
    }
    CalendarPanel {
        id: calendar
        anchors { top: bar.bottom; right: parent.right }
    }
    NotificationPopups {
        id: popups
        anchors { top: bar.bottom; right: parent.right }
    }
}
