// Snapping (Figma: "Settings - Snapping"): floating windows snap to the
// edges and corners of the screen, or with SHIFT held onto a monitor's own
// zones. Each monitor's zone grid is edited in place (ZoneEditor.qml). The
// compositor follows windows.snap and windows.snap.zones live.
import QtQuick
import qs
import qs.settings
import "Zones.js" as Z

Column {
    id: page
    spacing: 16

    readonly property var grids: Daemon.setting("windows.snap.zones", ({}))

    // Sets (or, for "", clears) one monitor's grid.
    function setIn(key, map, entry, value) {
        const m = Object.assign({}, map ?? {});
        if (value === "") delete m[entry];
        else m[entry] = value;
        Daemon.set(key, m);
    }
    function monitorName(o) {
        if (o.builtin) return "Built-in display";
        return [o.make, o.model].filter(s => s).join(" ") || o.connector;
    }

    PageHeader { title: "Snapping"; subtitle: "Drag a floating window to an edge or corner to fill part of the screen." }

    Card {
        SettingRow {
            title: "Snap windows"
            subtitle: "SUPER+Z or a WhiteRose app's maximise button shows the layouts"
            Toggle { checked: Daemon.setting("windows.snap", true); onToggled: v => Daemon.set("windows.snap", v) }
        }
    }

    Section { text: "Zones" }
    Card {
        SettingRow {
            title: "Each monitor"
            subtitle: "Hold SHIFT while dragging a window to drop it into a zone"
        }
        Repeater {
            model: Compositor.outputs
            delegate: Column {
                id: zonesFor
                required property var modelData
                readonly property var own: page.grids?.[modelData.id] ?? null
                width: parent.width
                Rectangle { width: parent.width; height: 1; color: Theme.surface2 }
                SettingRow {
                    divider: false
                    title: page.monitorName(zonesFor.modelData)
                    subtitle: zonesFor.modelData.connector + " · " + (zonesFor.own ? "Your zones" : "Two thirds and a third")
                    Button {
                        visible: !!zonesFor.own
                        text: "Reset"
                        onClicked: page.setIn("windows.snap.zones", page.grids, zonesFor.modelData.id, "")
                    }
                }
                ZoneEditor {
                    saved: zonesFor.own
                    ratio: Z.aspect(zonesFor.modelData, Theme.barHeight)
                    onSave: zones => page.setIn("windows.snap.zones", page.grids, zonesFor.modelData.id, zones)
                }
            }
        }
    }
}
