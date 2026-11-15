// Status cluster (Figma: Trigger/Status): network, Bluetooth, volume and
// battery, all live from NetworkManager, BlueZ, PipeWire and UPower over
// D-Bus signals; nothing here polls.
import QtQuick
import Quickshell.Bluetooth
import Quickshell.Networking
import Quickshell.Services.Pipewire
import Quickshell.Services.UPower

Trigger {
    id: root
    spacing: 7

    readonly property var sink: Pipewire.defaultAudioSink
    readonly property var battery: UPower.displayDevice
    readonly property bool hasBattery: battery !== null && battery.ready && battery.isLaptopBattery
    // UPower reports 0-1 in some versions and 0-100 in others; normalise.
    readonly property int percent: !hasBattery ? 0 : Math.round(battery.percentage <= 1 ? battery.percentage * 100 : battery.percentage)
    readonly property bool charging: hasBattery && battery.state === UPowerDeviceState.Charging

    // Network: the connected Wi-Fi network if any, else a connected wired device.
    readonly property var devices: Networking.devices.values
    readonly property var wifiDevice: devices.find(d => d.type === DeviceType.Wifi) ?? null
    readonly property var wifiNetwork: wifiDevice?.networks.values.find(n => n.connected) ?? null
    readonly property bool wired: devices.some(d => d.type === DeviceType.Wired && d.connected)
    readonly property string networkIcon: {
        if (wifiNetwork) {
            const s = wifiNetwork.signalStrength;
            return s >= 0.66 ? "wifi" : s >= 0.33 ? "wifiMid" : "wifiLow";
        }
        return wired ? "ethernet" : "wifiOff";
    }

    // Bluetooth: shown while the adapter is on; bright when something is connected.
    readonly property var btAdapter: Bluetooth.defaultAdapter
    readonly property bool btOn: btAdapter !== null && btAdapter.enabled
    readonly property bool btConnected: btOn && Bluetooth.devices.values.some(d => d.connected)

    // (Glyphs, 16px: placed on whole pixels so they stay sharp.)
    Glyph {
        y: Math.round((parent.height - height) / 2)
        visible: Networking.backend !== NetworkBackendType.None && Daemon.setting("bar.network", true)
        name: root.networkIcon
        color: root.wifiNetwork || root.wired ? Theme.text : Theme.muted
    }

    Glyph {
        y: Math.round((parent.height - height) / 2)
        visible: root.btOn && Daemon.setting("bar.bluetooth", true)
        name: "bluetooth"
        color: root.btConnected ? Theme.text : Theme.muted
    }

    // Keeps the default sink's properties (volume, mute) bound and live.
    PwObjectTracker { objects: root.sink ? [root.sink] : [] }

    Glyph {
        y: Math.round((parent.height - height) / 2)
        visible: root.sink !== null && Daemon.setting("bar.volume", true)
        name: root.sink?.audio?.muted ? "volumeMuted" : "volume"
        color: root.sink?.audio?.muted ? Theme.muted : Theme.text
    }

    // The battery: its body with the fill level inside, plus a bolt beside
    // it while charging.
    Row {
        y: Math.round((parent.height - height) / 2)
        visible: root.hasBattery
        spacing: 1
        Glyph {
            visible: root.charging
            name: "zap"
        }
        Glyph {
            id: batteryGlyph
            name: "battery"
            color: root.percent <= 15 && !root.charging ? Theme.rose : Theme.text
            // The level, in the body's inside less a pixel all round
            // (x 3-11, y 6-10), in whole pixels.
            Rectangle {
                x: 3; y: 6; height: 4
                width: Math.max(1, Math.round(8 * root.percent / 100))
                radius: 1
                color: batteryGlyph.color
            }
        }
    }

    Text {
        anchors.verticalCenter: parent.verticalCenter
        visible: root.hasBattery && Daemon.setting("bar.batteryPercent", true)
        text: root.percent + "%"
        color: Theme.text
        font { family: Theme.mono; pointSize: Theme.pt(11); weight: Font.Medium }
    }
}
