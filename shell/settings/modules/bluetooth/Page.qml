// Bluetooth: the adapter, your paired devices (connect, disconnect, forget,
// battery), and new ones nearby to pair. Searching runs only while this page
// is open.
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
    readonly property var nearby: devices.filter(d => !d.paired && !d.bonded && d.name !== "" && d.name !== d.address)

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
            visible: page.nearby.length === 0
            divider: false
            title: "Searching…"
            subtitle: "Put the device in pairing mode to see it here."
        }
        Repeater {
            model: page.nearby
            delegate: SettingRow {
                id: near
                required property var modelData
                required property int index
                divider: index > 0
                title: modelData.name
                subtitle: modelData.pairing ? "Pairing… confirm on the device if it asks" : ""
                Button {
                    text: near.modelData.pairing ? "Cancel" : "Pair"
                    kind: near.modelData.pairing ? "ghost" : "primary"
                    onClicked: near.modelData.pairing ? near.modelData.cancelPair() : near.modelData.pair()
                }
            }
        }
    }
}
