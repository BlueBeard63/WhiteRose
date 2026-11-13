// Windows (Figma: "Settings - Windows"): how windows are placed. The
// default layout, a layout per workspace and per monitor (a monitor's wins),
// and the tiling options. Snapping has its own page. The compositor follows
// the windows.* settings live.
import QtQuick
import Quickshell.Widgets
import qs
import qs.settings

Column {
    id: page
    spacing: 16

    readonly property string layout: Daemon.setting("windows.layout", "floating")
    readonly property var workspaces: Daemon.setting("windows.layout.workspaces", ({}))
    readonly property var monitors: Daemon.setting("windows.layout.monitors", ({}))
    readonly property var choices: [
        { label: "Default", value: "" },
        { label: "Floating", value: "floating" },
        { label: "Dwindle", value: "dwindle" },
        { label: "Master", value: "master" },
    ]

    // Sets (or, for "", clears) one entry of a layout map setting.
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
    function monitorSize(o) {
        const mm = o.physical_mm ?? [0, 0];
        const inches = Math.sqrt(mm[0] * mm[0] + mm[1] * mm[1]) / 25.4;
        return inches > 5 ? o.connector + " · " + inches.toFixed(1) + "″" : o.connector;
    }

    PageHeader { title: "Windows"; subtitle: "How windows are placed: where you put them, or tiled side by side." }

    // ---- the default layout ---------------------------------------------------------
    Section { text: "Layout" }
    // Three cards across where there's room, else two, else one.
    readonly property int cardColumns: width >= 3 * 150 + 24 ? 3 : width >= 2 * 150 + 12 ? 2 : 1
    Flow {
        width: parent.width
        spacing: 12
        Repeater {
            // Each preview's windows as fractions of the space below its bar.
            model: [
                { id: "floating", name: "Floating", say: "Where you put them", wins: [[0.14, 0.1, 0.48, 0.66], [0.4, 0.3, 0.46, 0.6]] },
                { id: "dwindle", name: "Dwindle", say: "Each new window splits the last",
                  wins: [[0, 0, 0.49, 1], [0.51, 0, 0.49, 0.48], [0.51, 0.52, 0.235, 0.48], [0.765, 0.52, 0.235, 0.48]] },
                { id: "master", name: "Master", say: "One large, the rest stacked", wins: [[0, 0, 0.55, 1], [0.57, 0, 0.43, 0.48], [0.57, 0.52, 0.43, 0.48]] },
            ]
            delegate: Rectangle {
                id: card
                required property var modelData
                readonly property bool on: page.layout === modelData.id
                Accessible.role: Accessible.RadioButton
                Accessible.name: modelData.name
                Accessible.description: modelData.say
                Accessible.checkable: true
                Accessible.checked: on
                Accessible.onPressAction: Daemon.set("windows.layout", modelData.id)
                width: (page.width - 12 * (page.cardColumns - 1)) / page.cardColumns
                height: 176
                radius: 16
                color: Theme.surface1
                border { width: on ? 1.5 : 0; color: Theme.pearl }

                // A small desktop: the wallpaper, the bar, the island and the windows.
                ClippingRectangle {
                    id: preview
                    x: 10; y: 10
                    width: parent.width - 20; height: 108
                    radius: 10
                    color: "black"
                    WallpaperView {
                        anchors.fill: parent
                        wallpaper: Wallpaper.current
                        sourceSize: Qt.size(400, 220)
                    }
                    Rectangle { width: parent.width; height: 7; color: "#0F0F0F" }
                    Item {
                        id: area
                        x: 8; y: 24; width: parent.width - 16; height: parent.height - 32
                        Repeater {
                            model: card.modelData.wins
                            delegate: Rectangle {
                                required property var modelData
                                x: modelData[0] * area.width; y: modelData[1] * area.height
                                width: modelData[2] * area.width; height: modelData[3] * area.height
                                radius: 4
                                color: Qt.rgba(0.11, 0.11, 0.12, 0.92)
                                border { width: 1; color: Qt.rgba(1, 1, 1, 0.1) }
                            }
                        }
                    }
                    Rectangle {
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: 62; height: 14; radius: 6
                        color: "#0F0F0F"
                    }
                }
                Row {
                    x: 14; y: preview.y + preview.height + 14
                    spacing: 10
                    Rectangle {
                        anchors.verticalCenter: parent.verticalCenter
                        width: 16; height: 16; radius: 8
                        color: "transparent"
                        border { width: 1.5; color: card.on ? Theme.pearl : Theme.muted }
                        Rectangle { anchors.centerIn: parent; visible: card.on; width: 8; height: 8; radius: 4; color: Theme.pearl }
                    }
                    Column {
                        spacing: 1
                        Text { text: card.modelData.name; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.Medium } }
                        Text {
                            text: card.modelData.say
                            color: Theme.muted
                            width: card.width - 50
                            elide: Text.ElideRight
                            font { family: Theme.font; pointSize: Theme.pt(11) }
                        }
                    }
                }
                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: Daemon.set("windows.layout", card.modelData.id) }
            }
        }
    }

    // ---- per workspace ----------------------------------------------------------------
    Section { text: "Workspaces" }
    Card {
        SettingRow {
            title: "Each workspace"
            subtitle: "Default follows the layout above; SUPER+T changes the workspace you're on"
        }
        Item {
            width: parent.width
            height: grid.height + 12
            Rectangle { width: parent.width; height: 1; color: Theme.surface2 }
            Grid {
                id: grid
                x: 14; y: 6
                width: parent.width - 28
                // Three across, fewer where narrow.
                columns: width >= 3 * 170 + 56 ? 3 : width >= 2 * 170 + 28 ? 2 : 1
                columnSpacing: 28
                Repeater {
                    model: 9
                    delegate: Item {
                        required property int index
                        readonly property string n: String(index + 1)
                        width: (grid.width - (grid.columns - 1) * grid.columnSpacing) / grid.columns
                        height: 44
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: "Workspace " + parent.n
                            color: Theme.text
                            font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.Medium }
                        }
                        Choice {
                            anchors.right: parent.right
                            label: "Workspace " + parent.n
                            options: page.choices
                            value: page.workspaces?.[parent.n] ?? ""
                            onPicked: v => page.setIn("windows.layout.workspaces", page.workspaces, parent.n, v)
                        }
                    }
                }
            }
        }
    }

    // ---- per monitor ------------------------------------------------------------------
    Section { text: "Monitors" }
    Card {
        SettingRow {
            title: "Each monitor"
            subtitle: "Wins over the workspace's layout, e.g. a portrait screen that always stacks"
        }
        Repeater {
            model: Compositor.outputs
            delegate: SettingRow {
                id: monitorRow
                required property var modelData
                title: page.monitorName(modelData)
                subtitle: page.monitorSize(modelData)
                Choice {
                    options: page.choices
                    value: page.monitors?.[monitorRow.modelData.id] ?? ""
                    onPicked: v => page.setIn("windows.layout.monitors", page.monitors, monitorRow.modelData.id, v)
                }
            }
        }
    }

    // ---- tiling ----------------------------------------------------------------------
    Section { text: "Tiling" }
    Card {
        Repeater {
            model: [
                { key: "windows.gaps.inner", title: "Gap between windows" },
                { key: "windows.gaps.outer", title: "Gap around the edge" },
            ]
            delegate: SettingRow {
                id: gapRow
                required property var modelData
                title: modelData.title
                readonly property int value: Daemon.setting(modelData.key, 8)
                property int shown: value
                onValueChanged: shown = value
                Slider {
                    implicitWidth: 180
                    value: gapRow.shown; maximum: 64
                    onMoved: v => { gapRow.shown = Math.round(v); gapSave.restart() }
                }
                Text { width: 48; horizontalAlignment: Text.AlignRight; anchors.verticalCenter: parent.verticalCenter; text: gapRow.shown + " px"; color: Theme.muted; font { family: Theme.mono; pointSize: Theme.pt(11.5) } }
                Timer { id: gapSave; interval: 250; onTriggered: Daemon.set(gapRow.modelData.key, gapRow.shown) }
            }
        }
        SettingRow {
            title: "No gaps with one window"
            subtitle: "A window on its own fills the screen"
            Toggle { checked: Daemon.setting("windows.gaps.smart", false); onToggled: v => Daemon.set("windows.gaps.smart", v) }
        }
        SettingRow {
            id: ratioRow
            title: "Master size"
            subtitle: "Master layout"
            readonly property real value: Daemon.setting("windows.master.ratio", 0.55)
            property real shown: value
            onValueChanged: shown = value
            Slider {
                implicitWidth: 180
                value: ratioRow.shown - 0.1; maximum: 0.8
                onMoved: v => { ratioRow.shown = Math.round((v + 0.1) * 20) / 20; ratioSave.restart() }
            }
            Text { width: 48; horizontalAlignment: Text.AlignRight; anchors.verticalCenter: parent.verticalCenter; text: Math.round(ratioRow.shown * 100) + "%"; color: Theme.muted; font { family: Theme.mono; pointSize: Theme.pt(11.5) } }
            Timer { id: ratioSave; interval: 250; onTriggered: Daemon.set("windows.master.ratio", ratioRow.shown) }
        }
        SettingRow {
            title: "Master on the"
            subtitle: "Master layout"
            Segmented {
                value: Daemon.setting("windows.master.side", "left")
                options: [{ label: "Left", value: "left" }, { label: "Right", value: "right" }, { label: "Top", value: "top" }, { label: "Bottom", value: "bottom" }]
                onPicked: v => Daemon.set("windows.master.side", v)
            }
        }
        SettingRow {
            title: "New windows split"
            subtitle: "Dwindle layout"
            Segmented {
                value: Daemon.setting("windows.dwindle.split", "auto")
                options: [{ label: "By shape", value: "auto" }, { label: "Side by side", value: "horizontal" }, { label: "Stacked", value: "vertical" }]
                onPicked: v => Daemon.set("windows.dwindle.split", v)
            }
        }
    }

    // ---- switching --------------------------------------------------------------------
    Section { text: "Switching" }
    Card {
        SettingRow {
            divider: false
            title: "ALT+TAB shows apps from"
            subtitle: "SUPER+TAB always shows every window"
            Segmented {
                value: Daemon.setting("windows.appSwitcher.workspaces", "all")
                options: [{ label: "All workspaces", value: "all" }, { label: "This workspace", value: "current" }]
                onPicked: v => Daemon.set("windows.appSwitcher.workspaces", v)
            }
        }
    }
}
