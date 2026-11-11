// Modules (Figma: "Settings - Modules"): every settings page is a module,
// loaded only while it's open. Core pages can't be turned off; others can.
// Installed modules (~/.local/share/whiterose/modules) show up here too.
import QtQuick
import Quickshell
import Quickshell.Io
import qs
import qs.settings

Column {
    id: page
    spacing: 16

    readonly property var modules: SettingsState.modules
    readonly property var disabled: Daemon.setting("settings.modules.disabled", [])
    readonly property int core: modules.filter(m => m.core).length

    // This process's memory (the whole Settings app).
    FileView { id: status; path: "/proc/self/status"; blockLoading: true }
    property real rssKb: 0
    function measure() { status.reload(); const m = /VmRSS:\s*(\d+)/.exec(status.text() ?? ""); page.rssKb = m ? parseInt(m[1]) : 0 }
    Component.onCompleted: measure()
    Timer { interval: 3000; running: true; repeat: true; onTriggered: page.measure() }

    function toggle(id, on) {
        Daemon.set("settings.modules.disabled", on ? page.disabled.filter(d => d !== id) : [...page.disabled, id]);
    }

    PageHeader { title: "Modules"; subtitle: "Every settings page is a module. Pages load only when opened and unload when closed." }

    Row {
        width: parent.width
        spacing: 10
        Repeater {
            model: [
                ["Installed", String(page.modules.length), page.core + " core · " + (page.modules.length - page.core) + " optional"],
                ["Loaded now", "1", SettingsState.current?.name ?? ""],
                ["Settings memory", Math.round(page.rssKb / 1024) + " MB", "Closed pages use none"],
            ]
            delegate: Rectangle {
                required property var modelData
                width: (page.width - 20) / 3; height: 76; radius: 16
                color: Theme.surface1
                Column {
                    x: 14; anchors.verticalCenter: parent.verticalCenter
                    spacing: 2
                    Text { text: modelData[0]; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
                    Text { text: modelData[1]; color: Theme.text; font { family: Theme.mono; pointSize: Theme.pt(18); weight: Font.Medium } }
                    Text { text: modelData[2]; color: Theme.faint; font { family: Theme.font; pointSize: Theme.pt(10.5) } }
                }
            }
        }
    }

    Section { text: "Installed" }
    Card {
        Repeater {
            model: page.modules
            delegate: Item {
                id: mod
                required property var modelData
                required property int index
                readonly property bool on: modelData.core || !page.disabled.includes(modelData.id)
                width: parent.width; height: 56
                opacity: on ? 1 : 0.5
                Rectangle { visible: mod.index > 0; width: parent.width; height: 1; color: Theme.surface2 }
                Rectangle {
                    id: modIcon
                    x: 14; anchors.verticalCenter: parent.verticalCenter
                    width: 30; height: 30; radius: 9
                    color: Theme.surface2
                    Icon { anchors.centerIn: parent; name: mod.modelData.icon; size: 15; color: Theme.text }
                }
                Column {
                    anchors { left: modIcon.right; leftMargin: 12; verticalCenter: parent.verticalCenter }
                    spacing: 2
                    Row {
                        spacing: 6
                        Text { text: mod.modelData.name; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.Medium } }
                        Badge { visible: mod.modelData.core; text: "core" }
                        Badge { visible: !mod.modelData.builtin; text: "installed" }
                        Text { text: "v" + (mod.modelData.version ?? "?"); color: Theme.faint; anchors.verticalCenter: parent.verticalCenter; font { family: Theme.mono; pointSize: Theme.pt(10) } }
                    }
                    Text { text: mod.modelData.description ?? ""; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11) } }
                }
                Row {
                    anchors { right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
                    spacing: 12
                    Row {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 5
                        Rectangle { visible: SettingsState.page === mod.modelData.id; width: 6; height: 6; radius: 3; color: Theme.rose; anchors.verticalCenter: parent.verticalCenter }
                        Text {
                            text: !mod.on ? "disabled" : SettingsState.page === mod.modelData.id ? "loaded" : "idle"
                            color: Theme.faint; font { family: Theme.mono; pointSize: Theme.pt(10.5) }
                        }
                    }
                    Toggle {
                        label: mod.modelData.name
                        checked: mod.on
                        enabled: !mod.modelData.core
                        onToggled: on => page.toggle(mod.modelData.id, on)
                    }
                }
            }
        }
    }
}
