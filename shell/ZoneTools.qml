// The on-screen zone editor's tools, in the island over the monitor being
// edited (Figma: "Edit zones on screen"): which monitor and whose grid, the
// same tools as Settings › Snapping, and Done (Escape too). The zones
// themselves are drawn by ZonesOverlay.qml; both follow ShellState.zones.
import QtQuick
import qs
import qs.settings
import "settings/modules/snapping/Zones.js" as Z

Item {
    id: root
    readonly property var zones: ShellState.zones
    readonly property var output: Compositor.outputs.find(o => o.id === root.zones?.monitor) ?? null

    implicitWidth: row.implicitWidth + 28
    implicitHeight: 49
    onWidthChanged: ShellState.zoneTools = Qt.size(width, height)
    onHeightChanged: ShellState.zoneTools = Qt.size(width, height)

    Row {
        id: row
        x: 18
        anchors.verticalCenter: parent.verticalCenter
        spacing: 12
        Column {
            anchors.verticalCenter: parent.verticalCenter
            spacing: 1
            Text { text: Z.monitorName(root.output); color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.Medium } }
            Text { text: root.zones?.ws ? "Workspace " + root.zones.ws : "All workspaces"; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
        }
        Segmented {
            anchors.verticalCenter: parent.verticalCenter
            value: root.zones?.tool ?? "resize"
            iconOnly: true
            options: Z.toolOptions()
            onPicked: v => ShellState.zonesTool(v)
        }
        Rectangle {
            Accessible.role: Accessible.Button
            Accessible.name: "Done"
            Accessible.onPressAction: ShellState.endZones()
            anchors.verticalCenter: parent.verticalCenter
            width: doneText.implicitWidth + 32; height: 31; radius: 10
            color: doneMouse.containsMouse ? Qt.lighter(Theme.pearl, 1.04) : Theme.pearl
            Text {
                id: doneText
                anchors.centerIn: parent
                text: "Done"
                color: Theme.bg
                font { family: Theme.font; pointSize: Theme.pt(12.5); weight: Font.Medium }
            }
            MouseArea { id: doneMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: ShellState.endZones() }
        }
    }
}
