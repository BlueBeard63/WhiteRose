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
                : page.dirty ? "Changes revert after " + page.trialSeconds + " s unless you keep them"
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
        subtitle: "WhiteRose recognises each monitor by its EDID, not the port it's plugged into."
        Choice {
            visible: page.several && page.caps.position
            value: page.profile
            options: page.profiles.map(p => ({ label: "Profile · " + p.name, value: p.name }))
                .concat(page.profiles.some(p => p.name === page.profile) ? [] : [{ label: "Profile · new", value: page.profile }])
            onPicked: v => page.useProfile(v)
        }
        Button { text: "Identify"; onClicked: Compositor.identifyOutputs() }
    }

    // ---- arrangement ------------------------------------------------------------
    // Drag a monitor to move it (edges snap together; hold Shift for fine
    // moves without snapping), or click one and nudge it with the arrow keys
    // (1 px, Shift 10 px). Tiles can't be dragged out of the card.
    Rectangle {
        id: arrange
        width: parent.width
        height: Math.round(Math.max(240, Math.min(380, width * 0.4)))
        radius: 16
        color: Theme.surface1
        border { width: activeFocus ? 1 : 0; color: Theme.surface2 }
        activeFocusOnTab: page.several && page.caps.position

        // Fit the layout (logical pixels) into the card, leaving room around
        // it to drag a monitor to any side of the others.
        readonly property var rects: page.draft.map(d => Object.assign({ on: d.enabled }, D.logicalSize(d), { x: d.position[0], y: d.position[1] }))
        readonly property real minX: Math.min(...rects.map(r => r.x))
        readonly property real minY: Math.min(...rects.map(r => r.y))
        readonly property real maxX: Math.max(...rects.map(r => r.x + r.w))
        readonly property real maxY: Math.max(...rects.map(r => r.y + r.h))
        // The bottom strip holds the hint.
        readonly property real hintHeight: 34
        readonly property real k: rects.length === 0 ? 0.05
            : Math.min(width * (page.several ? 0.6 : 0.8) / Math.max(1, maxX - minX),
                       (height - hintHeight - 24) * (page.several ? 0.62 : 0.8) / Math.max(1, maxY - minY), 0.15)
        readonly property real ox: (width - (maxX - minX) * k) / 2
        readonly property real oy: (height - hintHeight - (maxY - minY) * k) / 2
        // Snap distance: 12 px on screen, in logical pixels.
        readonly property real reach: 12 / Math.max(k, 0.001)

        function nudge(dx, dy) {
            const d = page.draft[page.selected];
            if (!d || !page.several || !page.caps.position) return;
            page.draft = D.place(page.draft, page.selected, d.position[0] + dx, d.position[1] + dy, 0);
        }
        Keys.onPressed: e => {
            const step = (e.modifiers & Qt.ShiftModifier) ? 10 : 1;
            const moves = { [Qt.Key_Left]: [-step, 0], [Qt.Key_Right]: [step, 0], [Qt.Key_Up]: [0, -step], [Qt.Key_Down]: [0, step] };
            const m = moves[e.key];
            if (m) { arrange.nudge(m[0], m[1]); e.accepted = true }
        }

        Repeater {
            model: page.draft
            delegate: Rectangle {
                id: tile
                required property var modelData
                required property int index
                readonly property var r: arrange.rects[index] ?? { x: 0, y: 0, w: 0, h: 0 }
                readonly property bool on: index === page.selected
                property bool dragging: false
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
                opacity: modelData.enabled ? (dragging ? 0.92 : 1) : 0.45
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
                    id: grab
                    readonly property bool movable: page.several && page.caps.position
                    // Where the press was (in the card) and where the tile was then.
                    property point from
                    property point start
                    anchors.fill: parent
                    // The page scrolls: without this an up/down drag scrolls it instead.
                    preventStealing: grab.movable
                    cursorShape: grab.movable ? (tile.dragging ? Qt.ClosedHandCursor : Qt.OpenHandCursor) : Qt.PointingHandCursor
                    onPressed: m => {
                        page.selected = tile.index;
                        arrange.forceActiveFocus();
                        grab.from = grab.mapToItem(arrange, m.x, m.y);
                        grab.start = Qt.point(tile.x, tile.y);
                    }
                    onPositionChanged: m => {
                        if (!grab.movable || !grab.pressed) return;
                        const at = grab.mapToItem(arrange, m.x, m.y);
                        const dx = at.x - grab.from.x, dy = at.y - grab.from.y;
                        if (!tile.dragging && Math.abs(dx) + Math.abs(dy) < 4) return;
                        tile.dragging = true;
                        const fine = (m.modifiers & Qt.ShiftModifier) !== 0;
                        const p = D.dragTo(grab.start.x, grab.start.y, dx, dy, fine,
                                           arrange.width - tile.width, arrange.height - arrange.hintHeight - tile.height);
                        const lx = (p.x - arrange.ox) / arrange.k + arrange.minX;
                        const ly = (p.y - arrange.oy) / arrange.k + arrange.minY;
                        const others = page.draft.filter((d, i) => i !== tile.index && d.enabled)
                            .map(d => Object.assign({ x: d.position[0], y: d.position[1] }, D.logicalSize(d)));
                        const snapped = fine ? { x: Math.round(lx), y: Math.round(ly) }
                            : D.snap({ x: lx, y: ly, w: tile.r.w, h: tile.r.h }, others, arrange.reach);
                        // Never on top of another monitor: shown sliding round it, where it will land.
                        const sn = D.resolveOverlap({ x: snapped.x, y: snapped.y, w: tile.r.w, h: tile.r.h }, others);
                        tile.preview = { x: Math.round(sn.x), y: Math.round(sn.y), fine: fine };
                        // Shown where it will land (snapped), still inside the card.
                        const c = D.dragTo(arrange.ox + (sn.x - arrange.minX) * arrange.k, arrange.oy + (sn.y - arrange.minY) * arrange.k,
                                           0, 0, false, arrange.width - tile.width, arrange.height - arrange.hintHeight - tile.height);
                        tile.x = c.x;
                        tile.y = c.y;
                    }
                    onReleased: {
                        const p = tile.preview;
                        tile.preview = null;
                        tile.dragging = false;
                        tile.follow();
                        // Last: a new draft makes new tiles (this one included).
                        if (p) page.draft = D.place(page.draft, tile.index, p.x, p.y, p.fine ? 0 : arrange.reach);
                    }
                    // Also sent as the tile goes away (a new draft): nothing to undo then.
                    onCanceled: if (tile.dragging) { tile.preview = null; tile.dragging = false; tile.follow() }
                }

                // The snapped position while dragging (Figma: "x 2560  y -24").
                Rectangle {
                    visible: tile.dragging && tile.preview !== null
                    anchors.horizontalCenter: parent.horizontalCenter
                    width: posText.implicitWidth + 12; height: 20; radius: 6
                    // Below the tile, or above it near the bottom of the card.
                    y: tile.y + tile.height + 28 < arrange.height - arrange.hintHeight ? parent.height + 6 : -height - 6
                    color: Theme.pearl
                    Text {
                        id: posText
                        anchors.centerIn: parent
                        text: tile.preview ? "x " + tile.preview.x + "   y " + tile.preview.y : ""
                        color: Theme.bg
                        font { family: Theme.mono; pointSize: Theme.pt(9.5) }
                    }
                }
            }
        }

        Text {
            anchors { left: parent.left; leftMargin: 16; right: parent.right; rightMargin: 16; bottom: parent.bottom; bottomMargin: 12 }
            elide: Text.ElideRight
            text: !page.several ? ""
                : !page.caps.position ? "This backend can't arrange monitors"
                : !D.connected(page.draft) ? "Every monitor must touch another, so the pointer can move between them"
                : "Drag to arrange · edges snap together · hold Shift for fine moves · arrow keys nudge the selected monitor"
            color: D.connected(page.draft) ? Theme.faint : Theme.rose
            font { family: Theme.font; pointSize: Theme.pt(10.5) }
        }
    }

    // ---- the selected monitor ---------------------------------------------------------
    Card {
        visible: page.current !== null
        // Header: name, model, how it's identified (narrow: the ID under the name).
        Item {
            id: monHeader
            readonly property bool narrow: page.width < 560
            width: parent.width; height: narrow ? 64 + 26 : 64
            Rectangle {
                id: monIcon
                x: 16; y: 15
                width: 34; height: 34; radius: 10
                color: Theme.surface2
                Icon { anchors.centerIn: parent; name: "monitor"; size: 16; color: Theme.text }
            }
            Column {
                id: monText
                anchors { left: monIcon.right; leftMargin: 12; right: monHeader.narrow ? parent.right : badges.left; rightMargin: monHeader.narrow ? 16 : 12; verticalCenter: monIcon.verticalCenter }
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
                x: monHeader.narrow ? monText.x : parent.width - 16 - width
                y: monHeader.narrow ? 60 : Math.round((64 - height) / 2)
                spacing: 6
                Badge { dot: false; text: "# " + (page.current?.id ?? "") }
                Item { width: 6; height: 1 }
                Item {
                    height: 22; width: strongRow.implicitWidth
                    Row {
                        id: strongRow
                        anchors.verticalCenter: parent.verticalCenter
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
                label: "Position x"
                implicitWidth: 86; mono: true
                text: String(page.current?.position[0] ?? 0)
                onAccepted: { const v = parseInt(text); if (!isNaN(v)) page.draft = D.place(page.draft, page.selected, v, page.current.position[1], 0) }
            }
            Field {
                label: "Position y"
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
            subtitle: "Below 100% fits more on screen"
            Segmented {
                readonly property var steps: [0.75, 1, 1.25, 1.5, 2]
                value: page.current?.scale ?? 1
                options: steps.concat(steps.includes(page.current?.scale ?? 1) ? [] : [page.current.scale]).sort((x, y) => x - y)
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
