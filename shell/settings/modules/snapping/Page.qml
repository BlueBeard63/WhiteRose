// Snapping (Figma: "Settings - Snapping"): floating windows snap to the
// edges and corners of the screen, or with SHIFT held onto a monitor's own
// zones. Each monitor's zone grid is edited in place (ZoneEditor.qml), with
// the tool picked in its header; "For" picks whose grid: all workspaces'
// (the monitor's), or one workspace's own. The compositor follows
// windows.snap, windows.snap.zones and windows.snap.zones.workspaces live.
import QtQuick
import Quickshell
import qs
import qs.settings
import "Zones.js" as Z

Column {
    id: page
    spacing: 16

    readonly property var grids: Daemon.setting("windows.snap.zones", ({}))
    // {"<workspace 1-9>": {"<monitor id>": grid}}: a workspace's own grids.
    readonly property var workspaceGrids: Daemon.setting("windows.snap.zones.workspaces", ({}))

    // Sets (or, for "", clears) workspace `ws`'s own grid on monitor `id`.
    function setWorkspace(ws, id, value) {
        const m = JSON.parse(JSON.stringify(page.workspaceGrids ?? {}));
        const own = m[ws] ?? {};
        if (value === "") delete own[id];
        else own[id] = value;
        if (Object.keys(own).length > 0) m[ws] = own;
        else delete m[ws];
        Daemon.set("windows.snap.zones.workspaces", m);
    }

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
                readonly property string monitor: modelData.id
                // "" for all workspaces (the monitor's grid), else "1"-"9".
                property string forWs: ""
                readonly property var monitorOwn: page.grids?.[monitor] ?? null
                readonly property var workspaceOwn: forWs ? page.workspaceGrids?.[forWs]?.[monitor] ?? null : null
                // What Reset clears and edits save to: the workspace's own, or the monitor's.
                readonly property var own: forWs ? workspaceOwn : monitorOwn
                // The grid shown (and edited from): a workspace without its own shows the monitor's.
                readonly property var shown: forWs ? workspaceOwn ?? monitorOwn : monitorOwn
                function save(zones) {
                    if (forWs) page.setWorkspace(forWs, monitor, zones);
                    else page.setIn("windows.snap.zones", page.grids, monitor, zones);
                }
                width: parent.width
                Rectangle { width: parent.width; height: 1; color: Theme.surface2 }
                SettingRow {
                    divider: false
                    title: page.monitorName(zonesFor.modelData)
                    subtitle: zonesFor.modelData.connector + " · " + (zonesFor.forWs
                        ? (zonesFor.workspaceOwn ? "Workspace " + zonesFor.forWs + "'s own zones" : "Same as all workspaces")
                        : (zonesFor.monitorOwn ? "Your zones" : "Two thirds and a third"))
                    // Whose grid: all workspaces', or one workspace's own.
                    Choice {
                        label: "Zones for"
                        value: zonesFor.forWs
                        options: [{ label: "All workspaces", value: "" }].concat([1, 2, 3, 4, 5, 6, 7, 8, 9].map(n => ({
                            label: "Workspace " + n + (page.workspaceGrids?.[String(n)]?.[zonesFor.monitor] ? " · own" : ""),
                            value: String(n),
                        })))
                        onPicked: v => zonesFor.forWs = v
                    }
                    // The editor's tool (it stays picked).
                    Segmented {
                        value: editor.tool
                        iconOnly: true
                        options: Z.toolOptions()
                        onPicked: v => editor.tool = v
                    }
                    Button {
                        visible: !!zonesFor.own
                        text: "Reset"
                        onClicked: zonesFor.save("")
                    }
                }
                // The editor, and "Edit on screen" beside it (under it where narrow).
                Item {
                    readonly property bool beside: editor.screenRight + 20 + onScreenSlot.width + 18 <= width
                    width: parent.width
                    height: editor.implicitHeight + (beside ? 0 : onScreenSlot.height + 8)
                    ZoneEditor {
                        id: editor
                        // A workspace without its own shows (and starts from) the monitor's.
                        saved: zonesFor.shown
                        ratio: Z.aspect(zonesFor.modelData, Theme.barHeight)
                        onSave: zones => zonesFor.save(zones)
                    }
                    // (In its own slot: a Button centres itself on its parent.)
                    Item {
                        id: onScreenSlot
                        x: parent.beside ? editor.screenRight + 20 : 18
                        y: parent.beside ? 4 : editor.implicitHeight - 8
                        width: onScreenButton.width; height: onScreenButton.height
                        Button {
                            id: onScreenButton
                            text: "Edit on screen"
                            icon: "monitor"
                            onClicked: onScreen.active = true
                        }
                    }
                }
                LazyLoader {
                    id: onScreen
                    active: false
                    ZonesOnScreen {
                        monitor: zonesFor.modelData
                        title: page.monitorName(zonesFor.modelData)
                        whose: zonesFor.forWs ? "Workspace " + zonesFor.forWs : "All workspaces"
                        saved: zonesFor.shown
                        Component.onCompleted: tool = editor.tool
                        onSave: zones => zonesFor.save(zones)
                        onDone: onScreen.active = false
                    }
                }
            }
        }
    }
}
