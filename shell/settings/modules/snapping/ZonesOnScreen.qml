// A monitor's zone grid edited at its real size, over that monitor (Figma:
// "Edit zones on screen"): the desktop dims behind translucent zones, and a
// tool bar under the island has the same tools as Settings plus Done
// (Escape too). Opened from the monitor's header in Settings › Snapping;
// it edits the grid picked there ("Zones for"), saving as it goes.
import QtQuick
import Quickshell
import Quickshell.Wayland
import qs
import qs.settings
import "Zones.js" as Z

PanelWindow {
    id: root
    // The Compositor.outputs entry it's for, and its name for the tool bar.
    property var monitor: null
    property string title: ""
    property string whose: ""
    property var saved: null
    property alias tool: editor.tool
    signal save(var zones)
    signal done()

    screen: Quickshell.screens.find(s => s.name === root.monitor?.connector) ?? null
    anchors { top: true; bottom: true; left: true; right: true }
    exclusionMode: ExclusionMode.Ignore
    color: "transparent"
    WlrLayershell.namespace: "whiterose-zone-editor"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
    LayerTitle { name: "whiterose-zone-editor" }
    // The bar stays usable: only the area below it takes the pointer.
    mask: Region { item: body }

    Item {
        id: body
        y: Theme.barHeight
        width: parent.width
        height: parent.height - Theme.barHeight
        focus: true
        Keys.onEscapePressed: root.done()

        Rectangle { anchors.fill: parent; color: Qt.rgba(0, 0, 0, 0.45) }

        ZoneEditor {
            id: editor
            anchors.fill: parent
            full: true
            saved: root.saved
            onSave: zones => root.save(zones)
        }

        // The tool bar, under the island.
        Rectangle {
            anchors.horizontalCenter: parent.horizontalCenter
            y: 34
            width: toolRow.implicitWidth + 28
            height: toolRow.implicitHeight + 16
            radius: 18
            color: Theme.bg
            Row {
                id: toolRow
                x: 18; anchors.verticalCenter: parent.verticalCenter
                spacing: 12
                Column {
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 1
                    Text { text: root.title; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.Medium } }
                    Text { text: root.whose; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
                }
                Segmented {
                    anchors.verticalCenter: parent.verticalCenter
                    value: editor.tool
                    iconOnly: true
                    options: Z.toolOptions()
                    onPicked: v => editor.tool = v
                }
                Rectangle {
                    Accessible.role: Accessible.Button
                    Accessible.name: "Done"
                    Accessible.onPressAction: root.done()
                    anchors.verticalCenter: parent.verticalCenter
                    width: doneText.implicitWidth + 32; height: 32; radius: 10
                    color: doneMouse.containsMouse ? Qt.lighter(Theme.pearl, 1.04) : Theme.pearl
                    Text {
                        id: doneText
                        anchors.centerIn: parent
                        text: "Done"
                        color: Theme.bg
                        font { family: Theme.font; pointSize: Theme.pt(12.5); weight: Font.Medium }
                    }
                    MouseArea { id: doneMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: root.done() }
                }
            }
        }
    }
}
