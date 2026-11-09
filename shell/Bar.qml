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
    implicitHeight: Theme.barHeight + island.height
    exclusiveZone: Theme.barHeight
    color: "transparent"

    WlrLayershell.namespace: "whiterose-bar"
    WlrLayershell.layer: WlrLayer.Top

    mask: Region {
        item: bar
        Region { item: island }
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
        // The dock panel comes next; for now clicking the grip just logs.
        onClicked: console.info("whiterose: dock requested")
    }

    Connections {
        target: Compositor
        function onAction(name) { console.info("whiterose: action", name, "(panel not built yet)") }
    }
}
