// Top Bar: what the bar shows, left to right. A preview at the top follows
// the switches; the real bar changes at the same moment.
import QtQuick
import qs
import qs.settings

Column {
    id: page
    spacing: 16

    function on(key) { return Daemon.setting(key, true) }
    readonly property bool numbers: Daemon.setting("bar.workspaceStyle", "dots") === "numbers"
    readonly property bool twelve: Daemon.setting("bar.timeFormat", "24h") === "12h"
    readonly property bool seconds: Daemon.setting("bar.seconds", false)
    // A fixed moment for the preview, so it doesn't tick.
    readonly property date sample: new Date(2026, 10, 9, 14, 32, 7)

    PageHeader { title: "Top Bar"; subtitle: "What the bar shows. Changes apply as you make them." }

    // ---- preview ------------------------------------------------------------------
    Rectangle {
        width: parent.width; height: 44; radius: 14
        color: Theme.surface1
        Rectangle {
            id: mock
            anchors { left: parent.left; right: parent.right; margins: 9; verticalCenter: parent.verticalCenter }
            height: 26; radius: 8
            color: Theme.bg
            clip: true
            Row {
                anchors { left: parent.left; leftMargin: 12; verticalCenter: parent.verticalCenter }
                spacing: 8
                Row {
                    visible: page.on("bar.workspaces")
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 5
                    Repeater {
                        model: 5
                        delegate: Rectangle {
                            required property int index
                            anchors.verticalCenter: parent.verticalCenter
                            width: page.numbers ? 16 : index === 0 ? 18 : 6
                            height: page.numbers ? 16 : 6
                            radius: page.numbers ? 5 : 3
                            color: page.numbers ? (index === 0 ? Theme.pearl : "transparent") : index === 0 ? Theme.pearl : index === 2 ? Theme.muted : Theme.faint
                            Text {
                                visible: page.numbers
                                anchors.centerIn: parent
                                text: index + 1
                                color: index === 0 ? Theme.bg : index === 2 ? Theme.text : Theme.faint
                                font { family: Theme.mono; pointSize: Theme.pt(10.5) }
                            }
                        }
                    }
                }
                Row {
                    // (Left out where the preview is too narrow for it.)
                    visible: page.on("bar.media") && mock.width >= 440
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 6
                    Row {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 1.5
                        Repeater { model: [6, 10, 4, 8]; delegate: Rectangle { required property int modelData; anchors.bottom: parent.bottom; width: 2; height: modelData; radius: 1; color: Theme.rose } }
                    }
                    Text { anchors.verticalCenter: parent.verticalCenter; text: "Midnight City"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
                    Text { anchors.verticalCenter: parent.verticalCenter; text: "M83"; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
                }
            }
            Row {
                anchors { right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter }
                spacing: 9
                Icon { visible: page.on("bar.network"); anchors.verticalCenter: parent.verticalCenter; name: "wifi" }
                Icon { visible: page.on("bar.bluetooth"); anchors.verticalCenter: parent.verticalCenter; name: "bluetooth" }
                Icon { visible: page.on("bar.volume"); anchors.verticalCenter: parent.verticalCenter; name: "volume" }
                Icon { anchors.verticalCenter: parent.verticalCenter; name: "battery"; size: 16 }
                Text { visible: page.on("bar.batteryPercent"); anchors.verticalCenter: parent.verticalCenter; text: "82%"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(11.5); weight: Font.Medium } }
                Icon { visible: page.on("bar.bell"); anchors.verticalCenter: parent.verticalCenter; name: "bell" }
                Text { visible: page.on("bar.date"); anchors.verticalCenter: parent.verticalCenter; text: Qt.formatDateTime(page.sample, "ddd d MMM"); color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: Qt.formatDateTime(page.sample, (page.twelve ? "h:mm" : "hh:mm") + (page.seconds ? ":ss" : "") + (page.twelve ? " AP" : ""))
                    color: Theme.text
                    font { family: Theme.mono; pointSize: Theme.pt(11.5); weight: Font.Medium }
                }
            }
        }
    }

    // ---- left ------------------------------------------------------------------------
    Section { text: "Left" }
    Card {
        SettingRow {
            title: "Workspaces"
            subtitle: "Click one to switch to it"
            Segmented {
                visible: page.on("bar.workspaces")
                value: Daemon.setting("bar.workspaceStyle", "dots")
                options: [{ label: "Dots", value: "dots" }, { label: "Numbers", value: "numbers" }]
                onPicked: v => Daemon.set("bar.workspaceStyle", v)
            }
            Toggle { checked: page.on("bar.workspaces"); onToggled: v => Daemon.set("bar.workspaces", v) }
        }
        SettingRow {
            title: "Now playing"
            subtitle: "The track and an equaliser while music plays; click to pause"
            Toggle { checked: page.on("bar.media"); onToggled: v => Daemon.set("bar.media", v) }
        }
    }

    // ---- right -----------------------------------------------------------------------
    Section { text: "Right" }
    Card {
        SettingRow {
            title: "Network"
            Toggle { checked: page.on("bar.network"); onToggled: v => Daemon.set("bar.network", v) }
        }
        SettingRow {
            title: "Bluetooth"
            subtitle: "Shown while Bluetooth is on"
            Toggle { checked: page.on("bar.bluetooth"); onToggled: v => Daemon.set("bar.bluetooth", v) }
        }
        SettingRow {
            title: "Volume"
            Toggle { checked: page.on("bar.volume"); onToggled: v => Daemon.set("bar.volume", v) }
        }
        SettingRow {
            title: "Battery percentage"
            subtitle: "The battery icon is always shown on laptops"
            Toggle { checked: page.on("bar.batteryPercent"); onToggled: v => Daemon.set("bar.batteryPercent", v) }
        }
        SettingRow {
            title: "Notifications"
            subtitle: "The bell; SUPER+N opens them either way"
            Toggle { checked: page.on("bar.bell"); onToggled: v => Daemon.set("bar.bell", v) }
        }
    }

    // ---- clock --------------------------------------------------------------------------
    Section { text: "Clock" }
    Card {
        SettingRow {
            title: "Date"
            Toggle { checked: page.on("bar.date"); onToggled: v => Daemon.set("bar.date", v) }
        }
        SettingRow {
            title: "Time"
            Segmented {
                value: Daemon.setting("bar.timeFormat", "24h")
                options: [{ label: "24-hour", value: "24h" }, { label: "12-hour", value: "12h" }]
                onPicked: v => Daemon.set("bar.timeFormat", v)
            }
        }
        SettingRow {
            title: "Seconds"
            subtitle: "Redraws the bar every second, a little more battery"
            Toggle { checked: page.seconds; onToggled: v => Daemon.set("bar.seconds", v) }
        }
    }

    // ---- screens -------------------------------------------------------------------------
    Card {
        SettingRow {
            title: "Show the bar on"
            subtitle: "With one screen this makes no difference"
            Segmented {
                value: Daemon.setting("bar.screens", "all")
                options: [{ label: "Every screen", value: "all" }, { label: "Main screen", value: "main" }]
                onPicked: v => Daemon.set("bar.screens", v)
            }
        }
    }
}
