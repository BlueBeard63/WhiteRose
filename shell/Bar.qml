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
    // Tall enough for the open dock while it is open or animating, so the
    // window never resizes mid-animation; otherwise just bar + idle island.
    implicitHeight: Theme.barHeight + (dock.expanded ? dock.openHeight + 30 : dock.height)
    exclusiveZone: Theme.barHeight
    color: "transparent"

    WlrLayershell.namespace: "whiterose-bar"
    WlrLayershell.layer: WlrLayer.Top
    // The open dock takes the keyboard (Esc closes it; clicking elsewhere
    // moves focus away, which closes it too).
    WlrLayershell.keyboardFocus: dock.open ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None

    mask: Region {
        item: bar
        Region { item: dock }
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

    Dock {
        id: dock
        anchors { top: bar.bottom; horizontalCenter: parent.horizontalCenter }
    }

    Connections {
        target: Compositor
        function onAction(name) { console.info("whiterose: action", name, "(panel not built yet)") }
    }
}
