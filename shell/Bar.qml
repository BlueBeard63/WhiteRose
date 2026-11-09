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
    implicitHeight: Theme.barHeight + island.reserve + 4
    exclusiveZone: Theme.barHeight
    color: "transparent"

    WlrLayershell.namespace: "whiterose-bar"
    WlrLayershell.layer: WlrLayer.Top
    // Open views take the keyboard (Esc closes; clicking elsewhere moves focus
    // away, which closes them too).
    WlrLayershell.keyboardFocus: island.wantsKeyboard ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

    mask: Region {
        item: bar
        Region { item: island }
        Region { item: island.previewCard }
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
            Status { anchors.verticalCenter: parent.verticalCenter }
            Trigger {
                anchors.verticalCenter: parent.verticalCenter
                Icon { name: "bell"; anchors.verticalCenter: parent.verticalCenter }
            }
            Clock { anchors.verticalCenter: parent.verticalCenter }
        }
    }

    Island {
        id: island
        anchors { top: bar.bottom; horizontalCenter: parent.horizontalCenter }
    }

    Connections {
        target: Compositor
        function onAction(name) {
            if (["quick_settings", "notifications", "screenshot_region"].includes(name))
                console.info("whiterose:", name, "panel is not built yet");
        }
    }
}
