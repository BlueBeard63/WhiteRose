// One monitor's zone grid (Figma: "Settings - Snapping"), edited in place
// with the tool picked in the monitor's header (it stays picked): Resize
// drags the lines between zones; Side by side and Stacked split the zone
// clicked (hovering shows where); Merge joins two zones clicked one after
// the other. Every change is saved as it's made; only grids the compositor
// takes are saved (Zones.js).
import QtQuick
import qs
import qs.settings
import "Zones.js" as Z

Item {
    id: ed
    // Its grid in windows.snap.zones, or null (it uses the default).
    property var saved: null
    // width / height of the monitor below its bar.
    property real ratio: 16 / 9
    signal save(var zones)

    // "resize", "side", "stacked" or "merge" (Zones.js TOOLS).
    property string tool: "resize"
    // While a line is dragged: the grid as it'll be.
    property var draft: null
    // The zone Merge picked first, or -1.
    property int picked: -1
    readonly property var zones: draft ?? saved ?? Z.fallback()
    onSavedChanged: picked = -1
    onToolChanged: picked = -1

    function commit(z) {
        if (z && Z.valid(z)) {
            picked = -1;
            ed.save(z);
        }
    }
    function clickZone(i) {
        const r = Z.click(zones, tool, picked, i);
        picked = r.picked;
        if (r.zones) commit(r.zones);
    }
    // What a click on a zone does now, for screen readers.
    readonly property string action: ({ side: "Split side by side", stacked: "Split stacked", merge: picked < 0 ? "Pick to merge" : "Merge" })[tool] ?? ""

    readonly property int gap: 4
    readonly property int bar: 7
    // The area below the bar: as wide as fits (up to 472), no taller than 260.
    readonly property real areaW: Math.max(120, Math.min(472, width - 36 - 2 * gap, 260 * ratio))
    readonly property real areaH: areaW / ratio

    width: parent?.width ?? 0
    implicitHeight: screen.height + 22

    Rectangle {
        id: screen
        x: 18; y: 4
        width: ed.areaW + 2 * ed.gap
        height: ed.areaH + ed.bar + 2 * ed.gap
        radius: 10
        color: "#0F0F0F"
        border { width: 1; color: Theme.surface2 }
        clip: true
        Rectangle { width: parent.width; height: ed.bar; color: "#08080A" }

        Item {
            id: area
            x: ed.gap; y: ed.bar + ed.gap
            width: ed.areaW; height: ed.areaH

            Repeater {
                model: ed.zones
                delegate: Rectangle {
                    id: zone
                    required property var modelData
                    required property int index
                    readonly property bool first: ed.picked === index
                    // Merge, a zone picked: only the ones it merges with respond.
                    readonly property bool takes: ed.tool !== "resize" && (ed.tool !== "merge" || ed.picked < 0 || first || Z.canMerge(ed.zones, ed.picked, index))
                    readonly property bool hot: zoneMouse.containsMouse && takes
                    readonly property bool splitting: hot && (ed.tool === "side" || ed.tool === "stacked")
                    // Half the gap off each side that meets another zone.
                    readonly property real x0: modelData[0] * area.width + (modelData[0] > 1e-6 ? ed.gap / 2 : 0)
                    readonly property real y0: modelData[1] * area.height + (modelData[1] > 1e-6 ? ed.gap / 2 : 0)
                    readonly property real x1: (modelData[0] + modelData[2]) * area.width - (modelData[0] + modelData[2] < 1 - 1e-6 ? ed.gap / 2 : 0)
                    readonly property real y1: (modelData[1] + modelData[3]) * area.height - (modelData[1] + modelData[3] < 1 - 1e-6 ? ed.gap / 2 : 0)
                    x: x0; y: y0; width: x1 - x0; height: y1 - y0
                    radius: 6
                    color: first ? Qt.rgba(Theme.rose.r, Theme.rose.g, Theme.rose.b, 0.35) : hot ? Theme.grip : Theme.surface2
                    border { width: first ? 1.5 : 0; color: Theme.pearl }
                    Accessible.role: Accessible.Button
                    Accessible.name: "Zone " + (index + 1)
                    Accessible.description: takes ? ed.action : ""
                    Accessible.onPressAction: if (takes) ed.clickZone(index)
                    Text {
                        anchors.centerIn: parent
                        // Out of the way of the split it would make.
                        anchors.horizontalCenterOffset: zone.splitting && ed.tool === "side" ? -zone.width / 4 : 0
                        anchors.verticalCenterOffset: zone.splitting && ed.tool === "stacked" ? -zone.height / 4 : 0
                        text: zone.index + 1
                        color: zone.first || zone.hot ? Theme.text : Theme.muted
                        font { family: Theme.mono; pointSize: Theme.pt(11) }
                    }
                    // Where Side by side or Stacked would split it.
                    Rectangle {
                        visible: zone.splitting
                        readonly property bool upright: ed.tool === "side"
                        x: upright ? zone.width / 2 - 1 : 8
                        y: upright ? 8 : zone.height / 2 - 1
                        width: upright ? 2 : zone.width - 16
                        height: upright ? zone.height - 16 : 2
                        radius: 1
                        color: Theme.pearl
                        opacity: 0.85
                    }
                    MouseArea {
                        id: zoneMouse
                        anchors.fill: parent
                        enabled: ed.tool !== "resize"
                        hoverEnabled: true
                        cursorShape: zone.takes ? Qt.PointingHandCursor : Qt.ArrowCursor
                        onClicked: if (zone.takes) ed.clickZone(zone.index)
                    }
                }
            }

            // The lines between zones: drag to move (with every zone on it).
            Repeater {
                model: Z.lines(ed.saved ?? Z.fallback())
                delegate: MouseArea {
                    id: line
                    required property var modelData
                    required property int index
                    readonly property bool upright: modelData.axis === "x"
                    readonly property real at: (upright ? area.width : area.height) * modelData.at
                    readonly property real from: (upright ? area.height : area.width) * modelData.lo
                    readonly property real to: (upright ? area.height : area.width) * modelData.hi
                    x: upright ? at - 6 : from
                    y: upright ? from : at - 6
                    width: upright ? 12 : to - from
                    height: upright ? to - from : 12
                    // Resize's: the other tools click the zones instead.
                    enabled: ed.tool === "resize"
                    hoverEnabled: true
                    preventStealing: true
                    cursorShape: upright ? Qt.SplitHCursor : Qt.SplitVCursor
                    Accessible.role: Accessible.Separator
                    Accessible.name: "Line " + (index + 1)
                    onPositionChanged: mouse => {
                        if (!pressed) return;
                        const p = mapToItem(area, mouse.x, mouse.y);
                        const f = upright ? p.x / area.width : p.y / area.height;
                        ed.draft = Z.moveLine(ed.saved ?? Z.fallback(), modelData.zone, modelData.dir, f);
                    }
                    onReleased: {
                        const z = ed.draft;
                        ed.draft = null;
                        if (z) ed.commit(z);
                    }
                    Rectangle {
                        anchors.centerIn: parent
                        visible: line.containsMouse || line.pressed
                        width: line.upright ? 4 : Math.min(40, parent.width)
                        height: line.upright ? Math.min(40, parent.height) : 4
                        radius: 2
                        color: Theme.pearl
                    }
                }
            }
        }
    }
}
