// Power & Battery (Figma: "Settings - Power & Battery"): the battery with its
// last 24 hours, power modes (power-profiles-daemon), the charge limit where
// the battery supports one, and the bar's percentage.
import QtQuick
import Quickshell
import Quickshell.Services.UPower
import qs
import qs.settings
import "../../../QuickSettings.js" as QS

Column {
    id: page
    spacing: 16

    readonly property var bat: UPower.displayDevice
    readonly property bool hasBattery: bat !== null && bat.ready && bat.isLaptopBattery
    readonly property int percent: !hasBattery ? 0 : Math.round(bat.percentage <= 1 ? bat.percentage * 100 : bat.percentage)
    readonly property string state: !hasBattery ? "" : bat.state === UPowerDeviceState.Charging ? "charging"
        : bat.state === UPowerDeviceState.FullyCharged ? "full" : "discharging"

    // Cycles, history and the charge limit come from whiterosed (UPower).
    property var info: null
    function refresh() { Daemon.request({ op: "battery" }, (v, err) => { if (!err) page.info = v }) }
    Component.onCompleted: refresh()
    Timer { interval: 60000; running: true; repeat: true; onTriggered: page.refresh() }

    PageHeader { title: "Power & Battery"; subtitle: "Laptop battery and power modes." }

    // ---- battery ------------------------------------------------------------------
    Card {
        visible: page.hasBattery
        Item {
            width: parent.width; height: 118
            Column {
                x: 18; anchors.verticalCenter: parent.verticalCenter
                spacing: 6
                Text { text: page.percent + "%"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(30); weight: Font.DemiBold } }
                Text {
                    text: page.state === "full" ? "Fully charged"
                        : page.state === "charging" ? (QS.duration(page.bat.timeToFull) ? QS.duration(page.bat.timeToFull) + " to full · charging" : "Charging")
                        : (QS.duration(page.bat.timeToEmpty) ? QS.duration(page.bat.timeToEmpty) + " remaining · on battery" : "On battery")
                    color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(12) }
                }
                Rectangle {
                    width: 200; height: 6; radius: 3; color: Theme.surface2
                    Rectangle { width: parent.width * page.percent / 100; height: 6; radius: 3; color: Theme.pearl }
                }
                Text {
                    visible: text !== ""
                    text: [page.info?.health ? "Health " + Math.round(page.info.health) + "%" : "",
                           page.info && page.info.cycles >= 0 ? page.info.cycles + " cycles" : ""].filter(s => s).join(" · ")
                    color: Theme.faint; font { family: Theme.font; pointSize: Theme.pt(11) }
                }
            }
            // The last 24 hours: rose while charging.
            Column {
                visible: (page.info?.history ?? []).length > 0
                anchors { right: parent.right; rightMargin: 18; verticalCenter: parent.verticalCenter }
                spacing: 6
                Row {
                    spacing: 3
                    height: 56
                    Repeater {
                        model: page.info?.history ?? []
                        delegate: Rectangle {
                            required property var modelData
                            anchors.bottom: parent.bottom
                            width: 6
                            height: modelData.percent < 0 ? 2 : Math.max(2, modelData.percent / 100 * 56)
                            radius: 2
                            color: modelData.percent < 0 ? Theme.surface2 : modelData.charging ? Theme.rose : Theme.grip
                        }
                    }
                }
                Row {
                    anchors.right: parent.right
                    spacing: 10
                    Row { spacing: 4; Rectangle { width: 6; height: 6; radius: 3; color: Theme.rose; anchors.verticalCenter: parent.verticalCenter } Text { text: "Charging"; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(10) } } }
                    Row { spacing: 4; Rectangle { width: 6; height: 6; radius: 3; color: Theme.grip; anchors.verticalCenter: parent.verticalCenter } Text { text: "On battery"; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(10) } } }
                    Text { text: "Last 24 h"; color: Theme.faint; font { family: Theme.font; pointSize: Theme.pt(10) } }
                }
            }
        }
    }

    // ---- power modes ----------------------------------------------------------------
    Row {
        width: parent.width
        spacing: 10
        Repeater {
            model: [
                { p: PowerProfile.PowerSaver, name: "Power saver", say: "Lower brightness, limits background work", icon: "leaf" },
                { p: PowerProfile.Balanced, name: "Balanced", say: "Best for most use", icon: "zap" },
                { p: PowerProfile.Performance, name: "Performance", say: "Highest speed, shorter battery life", icon: "gauge" },
            ].filter(m => m.p !== PowerProfile.Performance || PowerProfiles.hasPerformanceProfile)
            delegate: Rectangle {
                required property var modelData
                readonly property bool on: PowerProfiles.profile === modelData.p
                width: (page.width - 20) / 3; height: 64; radius: 16
                color: on ? Theme.surface2 : Theme.surface1
                border { width: on ? 1.5 : 0; color: Theme.pearl }
                Rectangle {
                    id: modeIcon
                    x: 12; anchors.verticalCenter: parent.verticalCenter
                    width: 32; height: 32; radius: 16
                    color: parent.on ? Theme.rose : Theme.surface2
                    Icon { anchors.centerIn: parent; name: modelData.icon; size: 15; color: parent.parent.on ? Theme.onAccent : Theme.text }
                }
                Column {
                    anchors { left: modeIcon.right; leftMargin: 10; right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
                    Text { text: modelData.name; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.Medium } }
                    Text { width: parent.width; wrapMode: Text.Wrap; text: modelData.say; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(10.5) } }
                }
                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: PowerProfiles.profile = modelData.p }
            }
        }
    }

    Card {
        SettingRow {
            divider: false
            visible: page.info?.limit_supported ?? false
            title: "Charge limit"
            subtitle: "Stops charging early to extend battery life" + (page.info?.limit_end ? " (at " + page.info.limit_end + "%)" : "")
            Toggle {
                checked: page.info?.limit_enabled ?? false
                onToggled: on => Daemon.request({ op: "set_charge_limit", on: on }, (v, err) => { if (err) console.warn(err); page.refresh() })
            }
        }
        SettingRow {
            divider: page.info?.limit_supported ?? false
            title: "Show percentage in top bar"
            Toggle {
                checked: Daemon.setting("bar.batteryPercent", true)
                onToggled: on => Daemon.set("bar.batteryPercent", on)
            }
        }
    }
}
