// One window rule (Figma: "Settings - Window rules"): its header says what
// it matches and does, with move up / down, delete, and open; open, it's
// edited in place, one row per thing it can set. Each change is sent up as
// `patched({ key: value })` (null removes the key). A pattern that doesn't
// compile shows why under its title and isn't sent until it does.
import QtQuick
import qs
import qs.settings
import "Rules.js" as R

Card {
    id: card
    required property var rule
    required property int index
    required property int count
    property bool open: false
    // A monitor id to its name.
    property var monitorName: id => id
    signal patched(var patch)
    signal moveBy(int by)
    signal removed()
    signal openToggled()

    readonly property string n: String(index + 1)

    // A field shows what's saved again when the rule changes under it
    // (saved, moved, or another rule in this place).
    onRuleChanged: {
        appField.text = rule.app ?? "";
        titleField.text = rule.title ?? "";
        appRow.problem = "";
        titleRow.problem = "";
        sizeRow.reset();
        atRow.reset();
    }

    SettingRow {
        title: R.name(card.rule)
        subtitle: R.summary(card.rule, card.index + 1, card.monitorName)
        Rectangle {
            anchors.verticalCenter: parent.verticalCenter
            width: order.implicitWidth + 4; height: 30; radius: 9
            color: Theme.surface2
            Row {
                id: order
                anchors.centerIn: parent
                spacing: 2
                IconButton {
                    width: 30; height: 26; radius: 8; icon: "chevUp"; iconSize: 15; iconColor: Theme.muted
                    fill: "transparent"; hoverFill: Theme.grip
                    label: "Move rule " + card.n + " up"
                    opacity: card.index > 0 ? 1 : 0.35
                    onClicked: if (card.index > 0) card.moveBy(-1)
                }
                IconButton {
                    width: 30; height: 26; radius: 8; icon: "chevDown"; iconSize: 15; iconColor: Theme.muted
                    fill: "transparent"; hoverFill: Theme.grip
                    label: "Move rule " + card.n + " down"
                    opacity: card.index < card.count - 1 ? 1 : 0.35
                    onClicked: if (card.index < card.count - 1) card.moveBy(1)
                }
                IconButton {
                    width: 30; height: 26; radius: 8; icon: "trash"; iconSize: 15; iconColor: Theme.muted
                    fill: "transparent"; hoverFill: Theme.grip
                    label: "Delete rule " + card.n
                    onClicked: card.removed()
                }
            }
        }
        IconButton {
            anchors.verticalCenter: parent.verticalCenter
            width: 30; height: 30; radius: 8; icon: card.open ? "chevUp" : "chevDown"; iconSize: 15
            fill: "transparent"
            label: (card.open ? "Close rule " : "Edit rule ") + card.n
            onClicked: card.openToggled()
        }
    }

    // ---- Open: what it matches
    SettingRow {
        id: appRow
        visible: card.open
        property string problem
        title: "App id"
        subtitle: problem || "A regular expression; it matches anywhere unless anchored with ^ and $"
        subtitleColor: problem ? Theme.rose : Theme.muted
        Field {
            id: appField
            mono: true
            placeholder: "Any app"
            error: appRow.problem !== ""
            text: card.rule.app ?? ""
            onEditingDone: {
                appRow.problem = R.patternError(text);
                if (!appRow.problem && text !== (card.rule.app ?? "")) card.patched({ app: text });
            }
        }
    }
    SettingRow {
        id: titleRow
        visible: card.open
        property string problem
        title: "Title"
        subtitle: problem || "The title it opens with; empty matches any title"
        subtitleColor: problem ? Theme.rose : Theme.muted
        Field {
            id: titleField
            mono: true
            placeholder: "Any title"
            error: titleRow.problem !== ""
            text: card.rule.title ?? ""
            onEditingDone: {
                titleRow.problem = R.patternError(text);
                if (!titleRow.problem && text !== (card.rule.title ?? "")) card.patched({ title: text });
            }
        }
    }

    // ---- Open: what it does
    SettingRow {
        visible: card.open
        title: "Workspace"
        subtitle: "It opens there without switching to it"
        Choice {
            value: card.rule.workspace ?? 0
            options: [{ label: "Where it opens", value: 0 }].concat([1, 2, 3, 4, 5, 6, 7, 8, 9].map(n => ({ label: "Workspace " + n, value: n })))
            onPicked: v => card.patched({ workspace: v || null })
        }
    }
    SettingRow {
        visible: card.open
        title: "Monitor"
        Choice {
            value: card.rule.monitor ?? ""
            options: {
                const connected = (Compositor.outputs ?? []).map(o => ({ label: card.monitorName(o.id), value: o.id }));
                const own = card.rule.monitor && !connected.some(o => o.value === card.rule.monitor)
                    ? [{ label: card.rule.monitor, value: card.rule.monitor, hint: "Unplugged" }] : [];
                return [{ label: "Where it opens", value: "" }].concat(connected, own);
            }
            onPicked: v => card.patched({ monitor: v || null })
        }
    }
    SettingRow {
        visible: card.open
        title: "Tiling"
        subtitle: "On a tiled workspace; dialogs and fixed-size windows always float"
        Segmented {
            value: R.tiling(card.rule)
            options: [{ label: "As the layout", value: "" }, { label: "Float", value: "float" }, { label: "Tile", value: "tile" }]
            onPicked: v => card.patched({ float: v === "" ? null : v === "float" })
        }
    }
    SettingRow {
        id: sizeRow
        visible: card.open
        readonly property bool on: R.isPair(card.rule.size)
        property bool bad: false
        function reset() {
            bad = false;
            sizeW.text = R.lengthText(card.rule.size?.[0]);
            sizeH.text = R.lengthText(card.rule.size?.[1]);
        }
        function commit() {
            const w = R.parseLength(sizeW.text, 1), h = R.parseLength(sizeH.text, 1);
            bad = w === null || h === null;
            if (!bad && (w !== card.rule.size?.[0] || h !== card.rule.size?.[1])) card.patched({ size: [w, h] });
        }
        title: "Size"
        subtitle: bad ? "Whole pixels (at least 1), or a % up to 100%" : "Pixels, or a % of the screen · floating windows only"
        subtitleColor: bad ? Theme.rose : Theme.muted
        Field {
            id: sizeW
            visible: sizeRow.on
            implicitWidth: 72
            mono: true
            label: "Width"
            error: sizeRow.bad
            text: R.lengthText(card.rule.size?.[0])
            onEditingDone: sizeRow.commit()
        }
        Text {
            visible: sizeRow.on
            anchors.verticalCenter: parent.verticalCenter
            text: "×"
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(12.5) }
        }
        Field {
            id: sizeH
            visible: sizeRow.on
            implicitWidth: 72
            mono: true
            label: "Height"
            error: sizeRow.bad
            text: R.lengthText(card.rule.size?.[1])
            onEditingDone: sizeRow.commit()
        }
        Toggle {
            checked: sizeRow.on
            onToggled: on => card.patched({ size: on ? ["50%", "50%"] : null })
        }
    }
    SettingRow {
        id: atRow
        visible: card.open
        readonly property bool at: R.position(card.rule) === "at"
        property bool bad: false
        function reset() {
            bad = false;
            atX.text = at ? R.lengthText(card.rule.position[0]) : "";
            atY.text = at ? R.lengthText(card.rule.position[1]) : "";
        }
        function commit() {
            const x = R.parseLength(atX.text, 0), y = R.parseLength(atY.text, 0);
            bad = x === null || y === null;
            if (!bad && (x !== card.rule.position?.[0] || y !== card.rule.position?.[1])) card.patched({ position: [x, y] });
        }
        title: "Position"
        subtitle: bad ? "Whole pixels, or a % up to 100%" : at ? "From the top left of the screen, below the bar" : "Floating windows only"
        subtitleColor: bad ? Theme.rose : Theme.muted
        Field {
            id: atX
            visible: atRow.at
            implicitWidth: 72
            mono: true
            label: "Across"
            error: atRow.bad
            text: atRow.at ? R.lengthText(card.rule.position[0]) : ""
            onEditingDone: atRow.commit()
        }
        Field {
            id: atY
            visible: atRow.at
            implicitWidth: 72
            mono: true
            label: "Down"
            error: atRow.bad
            text: atRow.at ? R.lengthText(card.rule.position[1]) : ""
            onEditingDone: atRow.commit()
        }
        Segmented {
            value: R.position(card.rule)
            options: [{ label: "Where it opens", value: "" }, { label: "Centred", value: "centre" }, { label: "At", value: "at" }]
            onPicked: v => card.patched({ position: v === "" ? null : v === "centre" ? "centre" : ["10%", "10%"] })
        }
    }
    SettingRow {
        visible: card.open
        title: "Zone"
        subtitle: "A snap zone, numbered as on the Snapping page · floating workspaces only"
        Choice {
            value: card.rule.zone ?? 0
            options: {
                const grids = JSON.parse(JSON.stringify(Daemon.setting("windows.snap.zones", ({})) ?? {}));
                const own = JSON.parse(JSON.stringify(Daemon.setting("windows.snap.zones.workspaces", ({})) ?? {}));
                const n = R.zoneCount(grids, own, card.rule.zone);
                const zones = [];
                for (let i = 1; i <= n; i++) zones.push({ label: "Zone " + i, value: i });
                return [{ label: "None", value: 0 }].concat(zones);
            }
            onPicked: v => card.patched({ zone: v || null })
        }
    }
    SettingRow {
        visible: card.open
        title: "Opens"
        Segmented {
            value: R.opens(card.rule)
            options: [{ label: "As it asks", value: "" }, { label: "Maximised", value: "maximized" }, { label: "Full screen", value: "fullscreen" }]
            onPicked: v => card.patched({ maximized: v === "maximized" || null, fullscreen: v === "fullscreen" || null })
        }
    }

    // ---- Open: how it looks
    SettingRow {
        id: opacityRow
        visible: card.open
        readonly property bool on: R.isPair(card.rule.opacity)
        title: "Opacity"
        subtitle: "How see-through it is · 100% is solid"
        Toggle {
            checked: opacityRow.on
            onToggled: on => card.patched({ opacity: on ? [1, 0.85] : null })
        }
    }
    Repeater {
        model: [{ title: "When focused", i: 0 }, { title: "When not focused", i: 1 }]
        delegate: SettingRow {
            id: levelRow
            required property var modelData
            visible: card.open && opacityRow.on
            readonly property real saved: opacityRow.on ? Number(card.rule.opacity[modelData.i]) : 1
            // While it's dragged (until saved): what it shows.
            property real dragged: -1
            readonly property real level: dragged >= 0 ? dragged : saved
            title: modelData.title
            Slider {
                implicitWidth: 180
                // 10%-100%.
                value: levelRow.level - 0.1
                maximum: 0.9
                onMoved: v => {
                    levelRow.dragged = Math.round((v + 0.1) * 20) / 20;
                    levelSave.restart();
                }
            }
            Text {
                width: 48
                horizontalAlignment: Text.AlignRight
                anchors.verticalCenter: parent.verticalCenter
                text: R.percent(levelRow.level)
                color: Theme.muted
                font { family: Theme.mono; pointSize: Theme.pt(11.5) }
            }
            Timer {
                id: levelSave
                interval: 250
                onTriggered: {
                    const both = [Number(card.rule.opacity[0]), Number(card.rule.opacity[1])];
                    both[levelRow.modelData.i] = levelRow.dragged;
                    levelRow.dragged = -1;
                    card.patched({ opacity: both });
                }
            }
        }
    }
    SettingRow {
        visible: card.open
        title: "Always on top"
        subtitle: "Above the other windows on its workspace"
        Toggle {
            checked: card.rule.onTop === true
            onToggled: on => card.patched({ onTop: on || null })
        }
    }
}
