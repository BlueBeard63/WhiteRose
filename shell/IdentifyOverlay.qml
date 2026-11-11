// "Identify" on the Displays page: each monitor shows its number (as on the
// page's arrangement) and name for three seconds. Clicks go straight through.
import QtQuick
import Quickshell
import Quickshell.Wayland

Scope {
    id: root
    property bool showing: false
    function show() { showing = true; hide.restart() }
    Timer { id: hide; interval: 3000; onTriggered: root.showing = false }

    Connections {
        target: Compositor
        function onAction(name) { if (name === "identify_outputs") root.show() }
    }

    Variants {
        model: root.showing ? Quickshell.screens : []
        delegate: PanelWindow {
            required property var modelData
            screen: modelData
            readonly property int index: Compositor.outputs.findIndex(o => o.connector === modelData.name)
            readonly property var output: Compositor.outputs[index] ?? null
            readonly property var saved: (Daemon.setting("displays", {}).monitor ?? []).find(m => m.id === output?.id)
            anchors { top: true; left: true }
            margins { top: 60; left: 40 }
            implicitWidth: card.width
            implicitHeight: card.height
            exclusionMode: ExclusionMode.Ignore
            color: "transparent"
            WlrLayershell.namespace: "whiterose-identify"
            LayerTitle { name: "whiterose-identify" }
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
            mask: Region {}

            Rectangle {
                id: card
                width: row.implicitWidth + 40
                height: 96
                radius: 24
                color: Theme.bg
                border { width: 1; color: Theme.surface2 }
                Row {
                    id: row
                    anchors.centerIn: parent
                    spacing: 16
                    Rectangle {
                        width: 56; height: 56; radius: 28
                        color: Theme.rose
                        Text {
                            anchors.centerIn: parent
                            text: index >= 0 ? index + 1 : "?"
                            color: Theme.onAccent
                            font { family: Theme.font; pointSize: Theme.pt(24); weight: Font.DemiBold }
                        }
                    }
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 2
                        Text {
                            text: saved?.label || (output?.builtin ? "Built-in" : output?.model || modelData.name)
                            color: Theme.text
                            font { family: Theme.font; pointSize: Theme.pt(18); weight: Font.DemiBold }
                        }
                        Text {
                            text: [output?.make, output?.connector].filter(s => s).join(" · ")
                            color: Theme.muted
                            font { family: Theme.font; pointSize: Theme.pt(12) }
                        }
                    }
                }
            }
        }
    }
}
