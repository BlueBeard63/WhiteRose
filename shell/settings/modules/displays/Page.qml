// Displays (Figma: "Settings - Displays", "… (dragging)"): the monitors as
// the compositor reports them, arranged by dragging (edges snap together),
// and each one's resolution, refresh rate, scale, rotation and variable
// refresh. Monitors are matched by their EDID identity, so settings follow a
// monitor whichever port it's on; the arrangement is kept per set of
// monitors (a profile).
//
// Apply tries the changes first: the compositor shows them for 15 seconds
// and goes back on its own unless you keep them (even if Settings crashed or
// the screen went black).
import QtQuick
import Quickshell
import qs
import qs.settings
import "Displays.js" as D

Column {
    id: page
    spacing: 16

    readonly property var outputs: Compositor.outputs
    readonly property var caps: Object.assign({ modes: false, rotation: false, position: false, disable: false, vrr: false }, Compositor.outputCaps)
    readonly property var saved: Daemon.setting("displays", ({ monitor: [], profile: [] }))
    readonly property var ids: outputs.map(o => o.id)
    readonly property bool several: outputs.length > 1

    // What's on screen now, and what the page is editing.
    property var base: []
    property var draft: []
    property int selected: 0
    readonly property var current: draft[selected] ?? null
    readonly property bool dirty: D.differs(base, draft)

    // The profile the layout is saved in ("" makes a new one).
    property string profile: ""
    readonly property var profiles: D.profilesFor(saved, ids)

    function reset() {
        page.base = D.draftFrom(page.outputs, page.saved);
        page.draft = D.draftFrom(page.outputs, page.saved);
        page.profile = page.outputs[0]?.profile ?? "";
        if (page.selected >= page.draft.length) page.selected = 0;
    }
    // Whatever changes underneath (hotplug, a trial starting or ending) is
    // shown as it is, unless you're in the middle of editing.
    onOutputsChanged: if (!dirty) reset()
    onSavedChanged: if (!dirty) reset()
    Component.onCompleted: reset()

    function edit(i, change) {
        const next = page.draft.slice();
        next[i] = Object.assign({}, next[i], change);
        page.draft = next;
    }

    // Lays the monitors out as `name` keeps them.
    function useProfile(name) {
        page.profile = name;
        const p = page.profiles.find(x => x.name === name);
        if (!p) return;
        page.draft = page.draft.map(d => {
            const o = p.output.find(x => x.id === d.id);
            return o ? Object.assign({}, d, { position: [o.position[0], o.position[1]], enabled: !o.disabled }) : d;
        });
    }

    // ---- trying and keeping ---------------------------------------------------
    readonly property int trialSeconds: 15
    property real now: Date.now()
    readonly property bool trying: Compositor.trialUntil > now
    readonly property int secondsLeft: Math.max(0, Math.ceil((Compositor.trialUntil - now) / 1000))
    Timer { interval: 250; repeat: true; running: Compositor.trialUntil > 0; onTriggered: page.now = Date.now() }
    // Kept or gone back: show what's in force now.
    onTryingChanged: if (!trying) reset()

    function apply() {
        const config = D.toConfig(page.saved, page.draft, page.profile);
        page.now = Date.now();
        Daemon.set("displays.test", { config: config, until: Date.now() + page.trialSeconds * 1000 });
    }
    function keep() {
        const t = Daemon.setting("displays.test", null);
        if (t?.config) Daemon.set("displays", t.config);
        Daemon.request({ op: "reset", key: "displays.test" });
    }
    function revert() {
        if (page.trying) Daemon.request({ op: "reset", key: "displays.test" });
        else page.reset();
    }

    property Component footer: Item {
        Text {
            anchors { left: parent.left; verticalCenter: parent.verticalCenter }
            text: page.trying ? "Going back to the previous settings in " + page.secondsLeft + " s unless you keep these"
                : page.dirty ? "Changes are tested first, then revert after " + page.trialSeconds + " s unless you confirm"
                : ""
            color: page.trying ? Theme.text : Theme.faint
            font { family: Theme.font; pointSize: Theme.pt(11.5) }
        }
        Row {
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            spacing: 8
            Button { text: "Revert"; enabled: page.trying || page.dirty; onClicked: page.revert() }
            Button {
                text: page.trying ? "Keep changes" : "Apply"
                kind: "primary"
                enabled: page.trying || (page.dirty && D.connected(page.draft))
                onClicked: page.trying ? page.keep() : page.apply()
            }
        }
    }

    PageHeader {
        title: "Displays"
        subtitle: "Monitors are matched by their EDID identity, not by the port they use."
        Choice {
            visible: page.several && page.caps.position
            value: page.profile
            options: page.profiles.map(p => ({ label: "Profile · " + p.name, value: p.name }))
                .concat(page.profiles.some(p => p.name === page.profile) ? [] : [{ label: "Profile · New", value: page.profile }])
            onPicked: v => page.useProfile(v)
        }
        Button { text: "Identify"; onClicked: Compositor.identifyOutputs() }
    }

    // ---- arrangement ------------------------------------------------------------
    Rectangle {
        id: arrange
        width: parent.width
        height: 170
        radius: 16
        color: Theme.surface1

        // Fit the layout (logical pixels) into the card.
        readonly property var rects: page.draft.map(d => Object.assign({ on: d.enabled }, D.logicalSize(d), { x: d.position[0], y: d.position[1] }))
        readonly property real minX: Math.min(...rects.map(r => r.x))
        readonly property real minY: Math.min(...rects.map(r => r.y))
        readonly property real maxX: Math.max(...rects.map(r => r.x + r.w))
        readonly property real maxY: Math.max(...rects.map(r => r.y + r.h))
        readonly property real k: rects.length === 0 ? 0.05
            : Math.min((width - 80) / Math.max(1, maxX - minX), (height - 64) / Math.max(1, maxY - minY), 0.08)
        readonly property real ox: (width - (maxX - minX) * k) / 2
        readonly property real oy: 18 + (height - 50 - (maxY - minY) * k) / 2
        // Snap distance: 14 px on screen, in logical pixels.
        readonly property real reach: 14 / Math.max(k, 0.001)

        Repeater {
            model: page.draft
            delegate: Rectangle {
                id: tile
                required property var modelData
                required property int index
                readonly property var r: arrange.rects[index] ?? { x: 0, y: 0, w: 0, h: 0 }
                readonly property bool on: index === page.selected
                readonly property bool dragging: drag.drag.active
                // Where the drag would put it (logical), snapped.
                property var preview: null
                // Dragging moves the tile by hand; afterwards it follows the layout again.
                function follow() {
                    x = Qt.binding(() => arrange.ox + (r.x - arrange.minX) * arrange.k);
                    y = Qt.binding(() => arrange.oy + (r.y - arrange.minY) * arrange.k);
                }
                Component.onCompleted: follow()
                width: Math.max(60, r.w * arrange.k)
                height: Math.max(40, r.h * arrange.k)
                radius: 10
                z: dragging ? 2 : on ? 1 : 0
                color: on ? Theme.tint(0.1) : Theme.bg
                opacity: modelData.enabled ? 1 : 0.45
                border { width: on ? 1.5 : 1; color: on ? Theme.pearl : Theme.surface2 }

                Column {
                    anchors.centerIn: parent
                    spacing: 3
                    width: parent.width - 12
                    Rectangle {
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: 16; height: 16; radius: 8
                        color: tile.on ? Theme.rose : Theme.surface2
                        Text {
                            anchors.centerIn: parent
                            text: tile.index + 1
                            color: tile.on ? Theme.onAccent : Theme.muted
                            font { family: Theme.font; pointSize: Theme.pt(8.5); weight: Font.DemiBold }
                        }
                    }
                    Text {
                        width: parent.width
                        horizontalAlignment: Text.AlignHCenter
                        elide: Text.ElideRight
                        text: tile.modelData.label
                        color: Theme.text
                        font { family: Theme.font; pointSize: Theme.pt(11.5); weight: Font.Medium }
                    }
                    Text {
                        visible: tile.height > 70
                        width: parent.width
                        horizontalAlignment: Text.AlignHCenter
                        elide: Text.ElideMiddle
                        text: tile.modelData.id
                        color: Theme.faint
                        font { family: Theme.mono; pointSize: Theme.pt(8.5) }
                    }
                }

                MouseArea {
                    id: drag
                    anchors.fill: parent
                    cursorShape: page.several && page.caps.position ? (drag.drag.active ? Qt.ClosedHandCursor : Qt.OpenHandCursor) : Qt.PointingHandCursor
                    drag.target: page.several && page.caps.position ? tile : null
                    drag.threshold: 4
                    onPressed: page.selected = tile.index
                    onPositionChanged: if (drag.drag.active) {
                        const lx = (tile.x - arrange.ox) / arrange.k + arrange.minX;
                        const ly = (tile.y - arrange.oy) / arrange.k + arrange.minY;
                        const others = page.draft.filter((d, i) => i !== tile.index && d.enabled)
                            .map(d => Object.assign({ x: d.position[0], y: d.position[1] }, D.logicalSize(d)));
                        tile.preview = D.snap({ x: lx, y: ly, w: tile.r.w, h: tile.r.h }, others, arrange.reach);
                    }
                    onReleased: {
                        const p = tile.preview;
                        tile.preview = null;
                        if (p) page.draft = D.place(page.draft, tile.index, p.x, p.y, arrange.reach);
                        tile.follow();
                    }
                }

                // The snapped position while dragging (Figma: "x 2560  y -24").
                Badge {
                    visible: tile.dragging && tile.preview !== null
                    anchors { verticalCenter: undefined; top: parent.bottom; topMargin: 6; horizontalCenter: parent.horizontalCenter }
                    tint: Theme.pearl; ink: Theme.bg
                    text: tile.preview ? "x " + Math.round(tile.preview.x) + "   y " + Math.round(tile.preview.y) : ""
                }
            }
        }

        Text {
            anchors { left: parent.left; leftMargin: 16; bottom: parent.bottom; bottomMargin: 12 }
            text: !page.several ? ""
                : !page.caps.position ? "This backend can't arrange monitors"
                : !D.connected(page.draft) ? "Every monitor must touch another, so the pointer can move between them"
                : "Drag to arrange · edges snap together"
            color: D.connected(page.draft) ? Theme.faint : Theme.rose
            font { family: Theme.font; pointSize: Theme.pt(10.5) }
        }
    }

    // ---- the selected monitor ---------------------------------------------------------
    Card {
        visible: page.current !== null
        // Header: name, model, how it's identified.
        Item {
            width: parent.width; height: 64
            Rectangle {
                id: monIcon
                x: 16; anchors.verticalCenter: parent.verticalCenter
                width: 34; height: 34; radius: 10
                color: Theme.surface2
                Icon { anchors.centerIn: parent; name: "monitor"; size: 16; color: Theme.text }
            }
            Column {
                anchors { left: monIcon.right; leftMargin: 12; right: badges.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
                spacing: 2
                Text {
                    width: parent.width; elide: Text.ElideRight
                    text: page.current?.label ?? ""
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
                }
                Text {
                    width: parent.width; elide: Text.ElideRight
                    text: [[page.current?.make, page.current?.model].filter(s => s).join(" "), page.current?.connector].filter(s => s).join(" · ")
                    color: Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(11.5) }
                }
            }
            Row {
                id: badges
                anchors { right: parent.right; rightMargin: 16; verticalCenter: parent.verticalCenter }
                spacing: 6
                Badge { height: 22; text: "# " + (page.current?.id ?? "") }
                Rectangle {
                    height: 22; width: strongRow.implicitWidth + 14; radius: 6
                    color: page.current?.strong ? Theme.tint(0.16) : Theme.surface2
                    Row {
                        id: strongRow
                        anchors.centerIn: parent
                        spacing: 5
                        Icon { anchors.verticalCenter: parent.verticalCenter; name: page.current?.strong ? "check" : "info"; size: 11; color: page.current?.strong ? Theme.rose : Theme.muted }
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            // Weak: no serial number in the EDID, or two identical
                            // monitors told apart by port.
                            text: page.current?.strong ? "Strong ID" : "Weak ID"
                            color: page.current?.strong ? Theme.rose : Theme.muted
                            font { family: Theme.font; pointSize: Theme.pt(10.5); weight: Font.Medium }
                        }
                    }
                }
            }
            Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.surface2 }
        }

        SettingRow {
            visible: page.several
            title: "Use this display"
            subtitle: page.caps.disable ? "" : "Can't be turned off here"
            dimmed: !page.caps.disable
            Toggle {
                checked: page.current?.enabled ?? true
                // At least one stays on.
                enabled: page.caps.disable && (!checked || page.draft.filter(d => d.enabled).length > 1)
                onToggled: on => page.edit(page.selected, { enabled: on })
            }
        }
        SettingRow {
            title: "Name"
            subtitle: "Shown here and when you identify displays"
            Field {
                implicitWidth: 200
                text: page.current?.label ?? ""
                onAccepted: page.edit(page.selected, { label: text.trim() || (page.current.builtin ? "Built-in" : page.current.model || page.current.connector) })
            }
        }
        SettingRow {
            visible: page.several && page.caps.position
            title: "Position"
            subtitle: "Logical pixels, top-left of this monitor"
            Field {
                implicitWidth: 86; mono: true
                text: String(page.current?.position[0] ?? 0)
                onAccepted: { const v = parseInt(text); if (!isNaN(v)) page.draft = D.place(page.draft, page.selected, v, page.current.position[1], 0) }
            }
            Field {
                implicitWidth: 86; mono: true
                text: String(page.current?.position[1] ?? 0)
                onAccepted: { const v = parseInt(text); if (!isNaN(v)) page.draft = D.place(page.draft, page.selected, page.current.position[0], v, 0) }
            }
        }
        SettingRow {
            readonly property var nativeRes: D.resolutions(page.current?.modes).find(r => r.preferred)
            title: "Resolution"
            subtitle: !page.caps.modes ? "Follows the window size" : nativeRes ? "Native " + nativeRes.width + " × " + nativeRes.height : ""
            dimmed: !page.caps.modes
            Choice {
                enabled: page.caps.modes
                value: page.current?.mode ? page.current.mode.width + "x" + page.current.mode.height : ""
                options: (page.caps.modes ? D.resolutions(page.current?.modes) : (page.current?.mode ? [page.current.mode] : []))
                    .map(r => ({ label: r.width + " × " + r.height, value: r.width + "x" + r.height }))
                onPicked: v => {
                    const [w, h] = v.split("x").map(Number);
                    page.edit(page.selected, { mode: D.withResolution(page.current.modes, w, h, page.current.mode?.refresh ?? 60000) });
                }
            }
        }
        SettingRow {
            title: "Refresh rate"
            dimmed: !page.caps.modes
            Choice {
                enabled: page.caps.modes
                value: page.current?.mode?.refresh ?? 0
                options: (page.current?.mode ? D.refreshes(page.current.modes, page.current.mode.width, page.current.mode.height) : [])
                    .map(r => ({ label: D.hzLabel(r), value: r }))
                onPicked: v => page.edit(page.selected, { mode: Object.assign({}, page.current.mode, { refresh: v }) })
            }
        }
        SettingRow {
            title: "Scale"
            subtitle: "Fractional scaling"
            Segmented {
                value: page.current?.scale ?? 1
                options: [1, 1.25, 1.5, 2].concat([1, 1.25, 1.5, 2].includes(page.current?.scale ?? 1) ? [] : [page.current.scale])
                    .map(s => ({ label: Math.round(s * 100) + "%", value: s }))
                onPicked: v => page.edit(page.selected, { scale: v })
            }
        }
        SettingRow {
            title: "Rotation"
            subtitle: page.caps.rotation ? "" : "Not available in a window"
            dimmed: !page.caps.rotation
            Choice {
                enabled: page.caps.rotation
                value: page.current?.transform ?? "normal"
                options: [
                    { label: "Normal", value: "normal" },
                    { label: "90°", value: "90" },
                    { label: "180°", value: "180" },
                    { label: "270°", value: "270" },
                ]
                onPicked: v => page.edit(page.selected, { transform: v })
            }
        }
        SettingRow {
            title: "Variable refresh rate"
            subtitle: page.caps.vrr ? "Lowers refresh on static content to save power" : "Not supported by these displays"
            dimmed: !page.caps.vrr
            Toggle {
                enabled: page.caps.vrr
                checked: page.current?.vrr ?? false
                onToggled: on => page.edit(page.selected, { vrr: on })
            }
        }
    }
}
