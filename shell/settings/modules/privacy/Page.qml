// Privacy: when the screen locks and turns off by itself, locking before the
// laptop sleeps, and what the desktop remembers (clipboard history, recent
// files in search, the launcher's history), with ways to forget it.
import QtQuick
import qs
import qs.settings

Column {
    id: page
    spacing: 16

    function minutes(m) { return m === 0 ? "Never" : m === 60 ? "After 1 hour" : m === 1 ? "After 1 minute" : "After " + m + " minutes" }
    readonly property var choices: [0, 1, 2, 5, 10, 15, 30, 60].map(m => ({ label: page.minutes(m), value: m }))
    readonly property int lockAfter: Daemon.setting("privacy.lockAfter", 5)
    readonly property int offAfter: Daemon.setting("privacy.screenOffAfter", 10)

    // The clipboard history's size (asked for when the page opens).
    Component.onCompleted: Compositor.clipboardList()
    readonly property int clips: Compositor.clipboard.length
    readonly property int remembered: Daemon.setting("launcher.commands", []).length
        + Daemon.setting("launcher.calculations", []).length + Object.keys(Daemon.setting("launcher.launches", ({}))).length

    PageHeader { title: "Privacy"; subtitle: "When the screen locks, and what the desktop remembers." }

    // ---- screen lock ---------------------------------------------------------------
    Section { text: "Screen lock" }
    Card {
        SettingRow {
            title: "Lock the screen"
            subtitle: "When you haven't used the keyboard or mouse for a while"
            Choice { value: page.lockAfter; options: page.choices; onPicked: v => Daemon.set("privacy.lockAfter", v) }
        }
        SettingRow {
            title: "Turn the screen off"
            subtitle: page.offAfter !== 0 && page.lockAfter !== 0 && page.offAfter < page.lockAfter
                ? "Before it locks: anyone can turn it back on and see your desktop"
                : "Saves battery; any key or touch turns it back on"
            Choice { value: page.offAfter; options: page.choices; onPicked: v => Daemon.set("privacy.screenOffAfter", v) }
        }
        SettingRow {
            title: "Lock before sleeping"
            subtitle: "Closing the lid or suspending locks first"
            Toggle { checked: Daemon.setting("privacy.lockOnSuspend", true); onToggled: v => Daemon.set("privacy.lockOnSuspend", v) }
        }
    }

    // ---- what's remembered ----------------------------------------------------------
    Section { text: "History" }
    Card {
        SettingRow {
            title: "Clipboard history"
            subtitle: Daemon.setting("privacy.clipboardHistory", true)
                ? (page.clips === 0 ? "Nothing remembered right now" : page.clips + (page.clips === 1 ? " item" : " items") + " in the island's clipboard view")
                : "Off: copying works, nothing is remembered"
            Button {
                visible: page.clips > 0
                text: "Clear"
                kind: "danger"
                onClicked: Compositor.clipboardClear()
            }
            Toggle { checked: Daemon.setting("privacy.clipboardHistory", true); onToggled: v => Daemon.set("privacy.clipboardHistory", v) }
        }
        SettingRow {
            title: "Recent files in search"
            subtitle: "The launcher shows files you opened recently"
            Toggle { checked: Daemon.setting("privacy.recentFiles", true); onToggled: v => Daemon.set("privacy.recentFiles", v) }
        }
        SettingRow {
            title: "Launcher history"
            subtitle: page.remembered === 0 ? "Nothing remembered" : "Recent commands, calculations and how often you open each app"
            Button {
                text: "Clear"
                kind: "danger"
                enabled: page.remembered > 0
                onClicked: {
                    Daemon.request({ op: "reset", key: "launcher.commands" });
                    Daemon.request({ op: "reset", key: "launcher.calculations" });
                    Daemon.request({ op: "reset", key: "launcher.launches" });
                }
            }
        }
    }
}
