// One monitor's zone grid (Figma: "Settings - Windows (snapping)"), edited
// in place: click a zone to choose it (a second one beside it, to merge
// them), split it side by side or stacked, drag the line between zones.
// Every change is saved as it's made; only grids the compositor takes are
// saved (Zones.js).
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

    // While a line is dragged: the grid as it'll be.
    property var draft: null
    property var selected: []
    readonly property var zones: draft ?? saved ?? Z.fallback()
    onSavedChanged: selected = []

    function commit(z) {
        if (z && Z.valid(z)) {
            selected = [];
            ed.save(z);
        }
    }

    readonly property int gap: 4
    readonly property int bar: 7
    // The area below the bar: as wide as fits, no taller than 200.
    readonly property real areaW: Math.min(328, 200 * ratio)
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
                    readonly property bool chosen: ed.selected.includes(index)
                    // Half the gap off each side that meets another zone.
                    readonly property real x0: modelData[0] * area.width + (modelData[0] > 1e-6 ? ed.gap / 2 : 0)
                    readonly property real y0: modelData[1] * area.height + (modelData[1] > 1e-6 ? ed.gap / 2 : 0)
                    readonly property real x1: (modelData[0] + modelData[2]) * area.width - (modelData[0] + modelData[2] < 1 - 1e-6 ? ed.gap / 2 : 0)
                    readonly property real y1: (modelData[1] + modelData[3]) * area.height - (modelData[1] + modelData[3] < 1 - 1e-6 ? ed.gap / 2 : 0)
                    x: x0; y: y0; width: x1 - x0; height: y1 - y0
                    radius: 6
                    color: chosen ? Qt.rgba(Theme.rose.r, Theme.rose.g, Theme.rose.b, 0.35) : zoneMouse.containsMouse ? Theme.grip : Theme.surface2
                    border { width: chosen ? 1.5 : 0; color: Theme.pearl }
                    Accessible.role: Accessible.CheckBox
                    Accessible.name: "Zone " + (index + 1)
                    Accessible.checkable: true
                    Accessible.checked: chosen
                    Accessible.onToggleAction: ed.selected = Z.select(ed.zones, ed.selected, index)
                    Accessible.onPressAction: ed.selected = Z.select(ed.zones, ed.selected, index)
                    Text {
                        anchors.centerIn: parent
                        text: zone.index + 1
                        color: zone.chosen ? Theme.text : Theme.muted
                        font { family: Theme.mono; pointSize: Theme.pt(11) }
                    }
                    MouseArea {
                        id: zoneMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: ed.selected = Z.select(ed.zones, ed.selected, zone.index)
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

    Column {
        anchors { left: screen.right; leftMargin: 20; top: screen.top }
        spacing: 8
        // (Each in its own slot: a Button centres itself on its parent.)
        Item {
            width: splitSide.width; height: splitSide.height
            Button {
                id: splitSide
                text: "Split side by side"
                enabled: ed.selected.length === 1
                onClicked: ed.commit(Z.split(ed.zones, ed.selected[0], "side"))
            }
        }
        Item {
            width: splitStacked.width; height: splitStacked.height
            Button {
                id: splitStacked
                text: "Split stacked"
                enabled: ed.selected.length === 1
                onClicked: ed.commit(Z.split(ed.zones, ed.selected[0], "stacked"))
            }
        }
        Item {
            width: mergeButton.width; height: mergeButton.height
            Button {
                id: mergeButton
                text: "Merge"
                enabled: ed.selected.length === 2
                onClicked: ed.commit(Z.merge(ed.zones, ed.selected[0], ed.selected[1]))
            }
        }
    }
}
