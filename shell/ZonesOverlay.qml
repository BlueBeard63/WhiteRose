// A monitor's snap zones edited at their real size, over that monitor
// (Figma: "Edit zones on screen"): the desktop dims behind translucent
// zones, with the tools in its island (island/zones.qml). Settings ›
// Snapping opens it ("Edit on screen", over IPC: see ShellState.zones) for
// the grid picked there, and it saves as it goes, as Settings does.
// Escape or Done ends it.
import QtQuick
import Quickshell
import Quickshell.Wayland
import qs
import "settings/modules/snapping"
import "settings/modules/snapping/Zones.js" as Z

Scope {
    LazyLoader {
        active: ShellState.zones !== null
        PanelWindow {
            id: root
            readonly property var zones: ShellState.zones
            readonly property var output: Compositor.outputs.find(o => o.id === root.zones?.monitor) ?? null
            readonly property var grids: Daemon.setting("windows.snap.zones", ({}))
            readonly property var wsGrids: Daemon.setting("windows.snap.zones.workspaces", ({}))

            screen: Quickshell.screens.find(s => s.name === root.output?.connector) ?? null
            anchors { top: true; bottom: true; left: true; right: true }
            exclusionMode: ExclusionMode.Ignore
            color: "transparent"
            WlrLayershell.namespace: "whiterose-zone-editor"
            // Above windows, under the bar (which rises to Overlay meanwhile)
            // so its island, with the tools, isn't dimmed. Over a fullscreen
            // window only Overlay is drawn, so there it goes over the bar.
            WlrLayershell.layer: Compositor.fullscreen.includes(root.output?.connector ?? "") ? WlrLayer.Overlay : WlrLayer.Top
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive
            LayerTitle { name: "whiterose-zone-editor" }
            // The bar and its island (the tools, hanging over the top of the
            // zones) stay usable: only the area below the bar, less the
            // island's (with its ears), takes the pointer.
            mask: Region {
                item: body
                Region {
                    intersection: Intersection.Subtract
                    readonly property real w: ShellState.zoneTools.width + 2 * 26
                    x: (root.width - w) / 2
                    y: 0
                    width: w
                    height: Theme.barHeight + ShellState.zoneTools.height
                }
            }

            // The monitor going away ends it.
            onOutputChanged: if (!output) ShellState.endZones()

            Item {
                id: body
                y: Theme.barHeight
                width: parent.width
                height: parent.height - Theme.barHeight
                focus: true
                Keys.onEscapePressed: ShellState.endZones()

                Rectangle { anchors.fill: parent; color: Qt.rgba(0, 0, 0, 0.45) }

                ZoneEditor {
                    anchors.fill: parent
                    full: true
                    tool: root.zones?.tool ?? "resize"
                    saved: Z.shown(root.grids, root.wsGrids, root.zones?.ws ?? "", root.zones?.monitor ?? "")
                    onSave: zones => {
                        const out = Z.save(root.grids, root.wsGrids, root.zones.ws, root.zones.monitor, zones);
                        Daemon.set(out.key, out.value);
                    }
                }
            }
        }
    }
}
