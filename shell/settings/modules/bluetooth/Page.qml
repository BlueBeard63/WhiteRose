// Bluetooth: the adapter, your paired devices (connect, disconnect, forget,
// battery), and new ones nearby to pair. Searching runs only while this page
// is open. Nearby lists only devices that say their name (most unnamed ones
// are beacons, trackers and other people's gadgets that can't be paired):
// accessories (headphones, keyboards, phones...) are shown; computers and
// devices that don't say what they are wait behind "Show others".
import QtQuick
import Quickshell
import Quickshell.Bluetooth
import qs
import qs.settings

Column {
    id: page
    spacing: 16

    readonly property var adapter: Bluetooth.defaultAdapter
    readonly property bool on: adapter?.enabled ?? false
    readonly property var devices: adapter?.devices.values ?? []
    readonly property var paired: devices.filter(d => d.paired || d.bonded).sort((a, b) => (b.connected - a.connected) || a.name.localeCompare(b.name))
    // Named, unpaired, not blocked: what's worth offering to pair.
    readonly property var named: devices.filter(d => !d.paired && !d.bonded && !d.blocked && (d.deviceName ?? "") !== "")
    // Accessories you'd pair (BlueZ's icon, from its device class or
    // appearance): sound, input, phones, cameras, printers. Computers
    // (other people's laptops, mostly) and ones that don't say are "others".
    function known(d) { return /^(audio|input|phone|camera|printer)/.test(d.icon ?? "") }
    readonly property var nearby: named.filter(d => known(d)).sort((a, b) => a.name.localeCompare(b.name))
    readonly property var others: named.filter(d => !known(d)).sort((a, b) => a.name.localeCompare(b.name))
    property bool showOthers: false
    readonly property var shown: nearby.concat(showOthers ? others : [])
    // What it is, from BlueZ's icon name ("audio-headphones", "input-mouse"...).
    function kind(d) {
        const i = d.icon ?? "";
        const names = [["audio-headset", "Headset"], ["audio-headphones", "Headphones"], ["audio", "Speaker"],
            ["input-keyboard", "Keyboard"], ["input-mouse", "Mouse"], ["input-tablet", "Tablet"], ["input-gaming", "Game controller"],
            ["input", "Input device"], ["phone", "Phone"], ["computer", "Computer"], ["camera", "Camera"], ["printer", "Printer"]];
        return (names.find(n => i.startsWith(n[0])) ?? ["", ""])[1];
    }

    function searching(on) { if (page.adapter && page.adapter.enabled) page.adapter.discovering = on }
    Component.onCompleted: searching(true)
    Component.onDestruction: searching(false)
    onOnChanged: searching(on)

    function stateText(d) {
        if (d.pairing) return "Pairing…";
        switch (d.state) {
        case BluetoothDeviceState.Connecting: return "Connecting…";
        case BluetoothDeviceState.Disconnecting: return "Disconnecting…";
        case BluetoothDeviceState.Connected: return "Connected" + (d.batteryAvailable ? " · battery " + Math.round(d.battery * 100) + "%" : "");
        }
        return "Not connected";
    }

    PageHeader { title: "Bluetooth"; subtitle: "Pairing and devices, through BlueZ." }

    Card {
        visible: page.adapter === null
        SettingRow { divider: false; title: "No Bluetooth adapter"; subtitle: "This computer has no Bluetooth, or it's switched off in the firmware." }
    }

    Card {
        visible: page.adapter !== null
        Item {
            width: parent.width; height: 64
            Rectangle {
                id: tile
                x: 14; anchors.verticalCenter: parent.verticalCenter
                width: 36; height: 36; radius: 10
                color: page.on ? Theme.rose : Theme.surface2
                Icon { anchors.centerIn: parent; name: "bluetooth"; size: 17; color: page.on ? Theme.onAccent : Theme.muted }
            }
            Column {
                anchors { left: tile.right; leftMargin: 12; verticalCenter: parent.verticalCenter }
                Text { text: "Bluetooth"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold } }
                Text {
                    text: page.on ? "Visible to others as “" + (page.adapter?.name ?? "") + "” while this page is open" : "Off"
                    color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) }
                }
            }
            Toggle {
                label: "Bluetooth"
                anchors { right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
                checked: page.on
                onToggled: on => page.adapter.enabled = on
            }
        }
    }

    Section { visible: page.on && page.paired.length > 0; text: "My devices" }
    Card {
        visible: page.on && page.paired.length > 0
        Repeater {
            model: page.paired
            delegate: SettingRow {
                id: dev
                required property var modelData
                required property int index
                divider: index > 0
                title: modelData.name
                subtitle: page.stateText(modelData)
                Button {
                    text: dev.modelData.connected ? "Disconnect" : "Connect"
                    kind: dev.modelData.connected ? "ghost" : "plain"
                    enabled: dev.modelData.state !== BluetoothDeviceState.Connecting && dev.modelData.state !== BluetoothDeviceState.Disconnecting
                    onClicked: dev.modelData.connected ? dev.modelData.disconnect() : dev.modelData.connect()
                }
                Button { text: "Forget"; kind: "danger"; onClicked: dev.modelData.forget() }
            }
        }
    }

    Section { visible: page.on; text: "Nearby" }
    Card {
        visible: page.on
        SettingRow {
            visible: page.shown.length === 0
            divider: false
            title: "Searching…"
            subtitle: "Put the device in pairing mode to see it here."
        }
        Repeater {
            model: page.shown
            delegate: SettingRow {
                id: near
                required property var modelData
                required property int index
                divider: index > 0
                title: modelData.name
                subtitle: modelData.pairing ? "Pairing… confirm on the device if it asks" : page.kind(modelData)
                Button {
                    text: near.modelData.pairing ? "Cancel" : "Pair"
                    kind: near.modelData.pairing ? "ghost" : "primary"
                    onClicked: near.modelData.pairing ? near.modelData.cancelPair() : near.modelData.pair()
                }
            }
        }
        SettingRow {
            visible: page.others.length > 0
            title: page.showOthers ? "Hide other devices" : "Show " + page.others.length + " other device" + (page.others.length === 1 ? "" : "s")
            subtitle: page.showOthers ? "" : "Computers, and devices that don't say what they are"
            Button { text: page.showOthers ? "Hide" : "Show"; kind: "ghost"; onClicked: page.showOthers = !page.showOthers }
        }
    }
}
