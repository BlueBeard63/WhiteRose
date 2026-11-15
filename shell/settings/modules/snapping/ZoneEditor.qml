// One monitor's zone grid (Figma: "Settings - Snapping"), edited in place
// with the tool picked in the monitor's header (it stays picked): Resize
// drags the lines between zones; Side by side and Stacked split the zone
// clicked (hovering shows where); Merge joins two zones clicked one after
// the other; Draw drags out a new zone over empty space; Delete takes the
// zone clicked away (never the last). Every change is saved as it's made;
// only grids the compositor takes are saved (Zones.js).
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
    readonly property string action: ({ side: "Split side by side", stacked: "Split stacked", merge: picked < 0 ? "Pick to merge" : "Merge", delete: "Delete" })[tool] ?? ""
    // Draw: where the drag started (fractions), and the zone it makes so far.
    property var drawFrom: null
    property var drawing: null
    // Red: a zone being deleted, or one drawn that won't fit.
    readonly property color danger: Qt.rgba(0.77, 0.17, 0.11, 0.45)

    // Full: the zones at their real size over the whole item (the shell's
    // editor on screen, ZonesOverlay.qml), translucent over the desktop;
    // otherwise a small monitor with its bar.
    property bool full: false
    readonly property int gap: full ? 16 : 4
    readonly property int bar: full ? 0 : 7
    // The area below the bar: as wide as fits (up to 472), no taller than 260.
    readonly property real areaW: full ? width - 2 * gap : Math.max(120, Math.min(472, width - 36 - 2 * gap, 260 * ratio))
    readonly property real areaH: full ? height - 2 * gap : areaW / ratio

    width: parent?.width ?? 0
    implicitHeight: screen.height + 22
    // Where the small monitor ends (things can go beside it).
    readonly property real screenRight: screen.x + screen.width

    Rectangle {
        id: screen
        x: ed.full ? 0 : 18; y: ed.full ? 0 : 4
        width: ed.areaW + 2 * ed.gap
        height: ed.areaH + ed.bar + 2 * ed.gap
        radius: ed.full ? 0 : 10
        color: ed.full ? "transparent" : "#0F0F0F"
        border { width: ed.full ? 0 : 1; color: Theme.surface2 }
        clip: true
        Rectangle { visible: !ed.full; width: parent.width; height: ed.bar; color: "#08080A" }

        Item {
            id: area
            x: ed.gap; y: ed.bar + ed.gap
            width: ed.areaW; height: ed.areaH

            // Draw: press in empty space and drag (the zones don't take
            // clicks with this tool, so presses on them land here too and
            // draw nothing).
            MouseArea {
                anchors.fill: parent
                enabled: ed.tool === "draw"
                preventStealing: true
                cursorShape: Qt.CrossCursor
                function at(mouse) { return [Math.min(1, Math.max(0, mouse.x / width)), Math.min(1, Math.max(0, mouse.y / height))] }
                onPressed: mouse => {
                    const p = at(mouse);
                    ed.drawFrom = Z.zoneAt(ed.zones, p[0], p[1]) < 0 ? p : null;
                    ed.drawing = ed.drawFrom ? Z.draw(ed.zones, p[0], p[1], p[0], p[1]) : null;
                }
                onPositionChanged: mouse => {
                    if (!ed.drawFrom) return;
                    const p = at(mouse);
                    ed.drawing = Z.draw(ed.zones, ed.drawFrom[0], ed.drawFrom[1], p[0], p[1]);
                }
                onReleased: {
                    const d = ed.drawing;
                    ed.drawFrom = null;
                    ed.drawing = null;
                    if (d?.ok) ed.commit(Z.add(ed.zones, d.zone));
                }
            }

            Repeater {
                model: ed.zones
                delegate: Rectangle {
                    id: zone
                    required property var modelData
                    required property int index
                    readonly property bool first: ed.picked === index
                    // Merge, a zone picked: only the ones it merges with respond.
                    readonly property bool takes: ed.tool !== "resize" && ed.tool !== "draw"
                        && (ed.tool !== "delete" || ed.zones.length > 1)
                        && (ed.tool !== "merge" || ed.picked < 0 || first || Z.canMerge(ed.zones, ed.picked, index))
                    readonly property bool hot: zoneMouse.containsMouse && takes
                    readonly property bool splitting: hot && (ed.tool === "side" || ed.tool === "stacked")
                    // Half the gap off each side that meets another zone.
                    readonly property real x0: modelData[0] * area.width + (modelData[0] > 1e-6 ? ed.gap / 2 : 0)
                    readonly property real y0: modelData[1] * area.height + (modelData[1] > 1e-6 ? ed.gap / 2 : 0)
                    readonly property real x1: (modelData[0] + modelData[2]) * area.width - (modelData[0] + modelData[2] < 1 - 1e-6 ? ed.gap / 2 : 0)
                    readonly property real y1: (modelData[1] + modelData[3]) * area.height - (modelData[1] + modelData[3] < 1 - 1e-6 ? ed.gap / 2 : 0)
                    x: x0; y: y0; width: x1 - x0; height: y1 - y0
                    radius: ed.full ? 16 : 6
                    readonly property color plain: hot ? Theme.grip : Theme.surface2
                    color: first ? Qt.rgba(Theme.rose.r, Theme.rose.g, Theme.rose.b, 0.35)
                        : hot && ed.tool === "delete" ? ed.danger
                        : Qt.rgba(plain.r, plain.g, plain.b, ed.full ? (hot ? 0.75 : 0.5) : 1)
                    border { width: first ? 1.5 : ed.full ? 1 : 0; color: first ? Theme.pearl : Qt.rgba(1, 1, 1, 0.22) }
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
                        font { family: Theme.mono; pointSize: Theme.pt(ed.full ? 26 : 11) }
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
                        enabled: ed.tool !== "resize" && ed.tool !== "draw"
                        hoverEnabled: true
                        cursorShape: zone.takes ? Qt.PointingHandCursor : Qt.ArrowCursor
                        onClicked: if (zone.takes) ed.clickZone(zone.index)
                    }
                }
            }

            // The zone being drawn: rose, or red if it won't fit.
            Rectangle {
                visible: ed.drawing !== null
                readonly property var drawn: ed.drawing?.zone ?? [0, 0, 0, 0]
                x: drawn[0] * area.width; y: drawn[1] * area.height
                width: drawn[2] * area.width; height: drawn[3] * area.height
                radius: 6
                color: ed.drawing?.ok ? Qt.rgba(Theme.rose.r, Theme.rose.g, Theme.rose.b, 0.3) : ed.danger
                border { width: 1.5; color: ed.drawing?.ok ? Theme.pearl : Qt.rgba(0.77, 0.17, 0.11, 1) }
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
