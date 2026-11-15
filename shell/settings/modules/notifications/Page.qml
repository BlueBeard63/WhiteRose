// Notifications: Do not disturb, how long popups stay, and per-app muting
// (a muted app's notifications go quietly into the panel). The app list is
// every app that has sent a notification.
import QtQuick
import Quickshell
import qs
import qs.settings

Column {
    id: page
    spacing: 16

    readonly property var apps: Daemon.setting("notifications.apps", [])
    readonly property var muted: Daemon.setting("notifications.muted", [])

    PageHeader { title: "Notifications"; subtitle: "What can interrupt you, and for how long." }

    Card {
        SettingRow {
            divider: false
            title: "Do not disturb"
            subtitle: "Notifications wait quietly in the panel; critical ones still show"
            Toggle { checked: QuickState.doNotDisturb; onToggled: on => QuickState.setDoNotDisturb(on) }
        }
        SettingRow {
            title: "Show popups for"
            Choice {
                value: Daemon.setting("notifications.popupSeconds", 5)
                options: [3, 5, 8, 12, 20, 30].map(s => ({ label: s + " seconds", value: s }))
                onPicked: v => Daemon.set("notifications.popupSeconds", v)
            }
        }
    }

    Section { text: "Apps" }
    Card {
        SettingRow {
            visible: page.apps.length === 0
            divider: false
            title: "No apps yet"
            subtitle: "Apps appear here once they've sent a notification."
        }
        Repeater {
            model: page.apps
            delegate: SettingRow {
                required property string modelData
                required property int index
                readonly property var entry: DesktopEntries.heuristicLookup(modelData)
                divider: index > 0
                title: entry?.name ?? modelData
                subtitle: page.muted.includes(modelData) ? "Muted: kept in the panel without a popup" : ""
                Toggle {
                    checked: !page.muted.includes(parent.parent.modelData)
                    onToggled: on => Daemon.set("notifications.muted", on
                        ? page.muted.filter(a => a !== parent.parent.modelData)
                        : [...page.muted, parent.parent.modelData])
                }
            }
        }
    }
}
