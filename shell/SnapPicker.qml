// The snap layout picker (SUPER+Z, a drag to the middle of the top edge, a
// WhiteRose app's maximize button): the layouts as small diagrams, from
// the Figma frames "Snap Picker - SUPER + Z" and "Snap Picker - Maximize
// button". The compositor places it, finds the cell under the pointer (even
// mid-drag) and snaps the window; this only draws what it sends
// (`snap_picker`), exactly where it says.
import QtQuick
import Quickshell
import Quickshell.Wayland

Scope {
    id: root
    readonly property var picker: Compositor.snapPicker
    // Hanging from the bar, it has the open island's ears.
    readonly property int earWidth: 26
    readonly property int earHeight: 30

    function layoutName(id) {
        return ({
            "halves": "Halves",
            "thirds": "Thirds",
            "quarters": "Quarters",
            "two-thirds-left": "Two thirds left",
            "two-thirds-right": "Two thirds right",
            "zones": "Your zones",
        })[id] ?? id;
    }

    Variants {
        model: root.picker ? Quickshell.screens.filter(s => s.name === root.picker.output) : []
        delegate: PanelWindow {
            id: win
            required property var modelData
            readonly property var p: root.picker
            readonly property int ear: p && !p.card ? root.earWidth : 0
            screen: modelData
            anchors { top: true; left: true }
            margins { top: p ? p.y : 0; left: p ? p.x - ear : 0 }
            implicitWidth: p ? p.width + 2 * ear : 1
            implicitHeight: p ? p.height : 1
            exclusionMode: ExclusionMode.Ignore
            color: "transparent"
            WlrLayershell.namespace: "whiterose-snap-picker"
            LayerTitle { name: "whiterose-snap-picker" }
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.None

            PanelShape {
                visible: win.p && !win.p.card
                bodyWidth: win.p ? win.p.width : 0
                bodyHeight: win.p ? win.p.height : 0
                earWidth: root.earWidth
                earHeight: root.earHeight
                bottomLeftRadius: 40
                bottomRightRadius: 40
            }
            Rectangle {
                visible: win.p && win.p.card
                anchors.fill: parent
                radius: 18
                color: Theme.bg
                border { width: 1; color: Theme.surface2 }
            }

            Item {
                id: body
                x: win.ear
                width: win.p ? win.p.width : 0
                height: win.p ? win.p.height : 0

                Row {
                    x: 24; y: 18
                    spacing: 8
                    Text {
                        text: "Snap"
                        color: Theme.text
                        font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
                    }
                    Text {
                        anchors.baseline: parent.children[0].baseline
                        width: Math.min(implicitWidth, body.width - 120)
                        elide: Text.ElideRight
                        text: win.p ? win.p.title : ""
                        color: Theme.muted
                        font { family: Theme.font; pointSize: Theme.pt(12) }
                    }
                }

                Repeater {
                    model: win.p ? win.p.layouts : []
                    delegate: Rectangle {
                        id: tile
                        required property var modelData
                        required property int index
                        readonly property bool chosen: !!win.p && !!win.p.hovered && win.p.hovered[0] === index
                        x: modelData.rect[0]; y: modelData.rect[1]
                        width: modelData.rect[2]; height: modelData.rect[3]
                        radius: 14
                        color: Theme.surface1
                        border { width: chosen ? 1.5 : 0; color: Theme.pearl }

                        Repeater {
                            model: tile.modelData.cells
                            delegate: Rectangle {
                                required property var modelData
                                required property int index
                                readonly property bool chosen: tile.chosen && win.p.hovered[1] === index
                                x: modelData[0] - tile.x; y: modelData[1] - tile.y
                                width: modelData[2]; height: modelData[3]
                                radius: 6
                                color: chosen ? Theme.rose : Theme.surface2
                                Accessible.role: Accessible.Button
                                Accessible.name: root.layoutName(tile.modelData.id) + " " + (index + 1)
                                Accessible.description: chosen ? "Chosen" : ""
                            }
                        }
                    }
                }
            }
        }
    }
}
