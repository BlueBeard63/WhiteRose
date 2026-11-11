// Quick settings (Figma: "Desktop - Quick Settings"): hangs from the right end
// of the bar. Opened from the status cluster or SUPER+S. Wi-Fi, Bluetooth,
// volume and battery come from QuickShell's services (NetworkManager, BlueZ,
// PipeWire, UPower); brightness and airplane mode from whiterosed; Night
// Light from the compositor; Do Not Disturb belongs to the shell.
import QtQuick
import Quickshell
import Quickshell.Bluetooth
import Quickshell.Io
import Quickshell.Networking
import Quickshell.Services.Pipewire
import Quickshell.Services.UPower
import Quickshell.Widgets
import "QuickSettings.js" as QS

SidePanel {
    id: root
    name: "quick"

    property bool powerMenu: false
    onOpenChanged: {
        powerMenu = false;
        if (open) uptimeFile.reload();
    }
    // Esc closes the power menu first.
    escapeHandler: () => {
        if (!powerMenu) return false;
        powerMenu = false;
        return true;
    }

    // ---- live state ---------------------------------------------------------
    readonly property var wifiDevice: Networking.devices.values.find(d => d.type === DeviceType.Wifi) ?? null
    readonly property var wifiNetwork: wifiDevice?.networks.values.find(n => n.connected) ?? null
    readonly property bool wired: Networking.devices.values.some(d => d.type === DeviceType.Wired && d.connected)

    readonly property var bt: Bluetooth.defaultAdapter
    readonly property var btDevice: bt?.devices.values.find(d => d.connected) ?? null

    readonly property var sink: Pipewire.defaultAudioSink
    PwObjectTracker { objects: root.sink ? [root.sink] : [] }

    readonly property var battery: UPower.displayDevice
    readonly property bool hasBattery: battery !== null && battery.ready && battery.isLaptopBattery
    readonly property int percent: !hasBattery ? 0 : Math.round(battery.percentage <= 1 ? battery.percentage * 100 : battery.percentage)
    readonly property string batteryState: !hasBattery ? "" :
        battery.state === UPowerDeviceState.Charging ? "charging" :
        battery.state === UPowerDeviceState.FullyCharged ? "full" : "discharging"

    readonly property string user: Quickshell.env("USER") ?? ""
    FileView { id: passwd; path: "/etc/passwd" }
    readonly property string userName: QS.displayName(passwd.text(), user)
    FileView { id: uptimeFile; path: "/proc/uptime"; blockLoading: true }
    readonly property string uptime: QS.uptime(parseFloat(uptimeFile.text()) || 0)
    // AccountsService's picture (what GNOME's settings sets), else ~/.face.
    readonly property var avatars: ["/var/lib/AccountsService/icons/" + user, Quickshell.env("HOME") + "/.face"]

    // Header: who's logged in, uptime, lock and power.
    Item {
        width: parent.width
        height: 36
        Row {
            spacing: 10
            anchors.verticalCenter: parent.verticalCenter
            // The user's picture, or their initials on a gradient.
            ClippingRectangle {
                width: 34; height: 34; radius: 17
                Rectangle {
                    anchors.fill: parent
                    gradient: Gradient {
                        GradientStop { position: 0; color: Theme.rose }
                        GradientStop { position: 1; color: "#6B4A8C" }
                    }
                }
                Text {
                    anchors.centerIn: parent
                    visible: picture.status !== Image.Ready
                    text: QS.initials(root.userName)
                    color: Theme.bg
                    font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
                }
                Image {
                    id: picture
                    property int tried: 0
                    anchors.fill: parent
                    source: "file://" + root.avatars[tried]
                    sourceSize: Qt.size(68, 68)
                    fillMode: Image.PreserveAspectCrop
                    visible: status === Image.Ready
                    onStatusChanged: if (status === Image.Error && tried + 1 < root.avatars.length) tried++
                }
            }
            Column {
                anchors.verticalCenter: parent.verticalCenter
                spacing: 1
                Text {
                    text: root.userName
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
                }
                Text {
                    text: root.uptime
                    color: Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(11.5) }
                }
            }
        }
        Row {
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            spacing: 6
            IconButton {
                width: 30; height: 30; icon: "lock"
                onClicked: { ShellState.closePanel(); ShellState.lock() }
            }
            IconButton {
                width: 30; height: 30; icon: "gear"
                onClicked: ShellState.openSettings("")
            }
            IconButton {
                width: 30; height: 30; icon: "power"
                iconColor: root.powerMenu ? Theme.onAccent : Theme.rose
                fill: root.powerMenu ? Theme.rose : Theme.surface1
                hoverFill: root.powerMenu ? Theme.rose : Theme.surface2
                onClicked: root.powerMenu = !root.powerMenu
            }
        }
    }

    // Toggles, or the power menu in their place.
    Grid {
        visible: !root.powerMenu
        columns: 2
        spacing: 8
        QsTile {
            visible: Networking.backend !== NetworkBackendType.None
            icon: Networking.wifiEnabled ? "wifi" : "wifiOff"
            title: "Wi-Fi"
            active: Networking.wifiEnabled
            subtitle: !Networking.wifiEnabled ? "Off" : root.wifiNetwork?.name ?? (root.wired ? "Wired" : "Not connected")
            onClicked: Networking.wifiEnabled = !Networking.wifiEnabled
        }
        QsTile {
            visible: root.bt !== null
            icon: "bluetooth"
            title: "Bluetooth"
            active: root.bt?.enabled ?? false
            subtitle: !(root.bt?.enabled) ? "Off" : root.btDevice?.name ?? "On"
            onClicked: root.bt.enabled = !root.bt.enabled
        }
        QsTile {
            icon: "bellOff"
            title: "Do Not Disturb"
            active: QuickState.doNotDisturb
            subtitle: active ? "On" : "Off"
            onClicked: QuickState.setDoNotDisturb(!active)
        }
        QsTile {
            icon: "moon"
            title: "Night Light"
            active: QuickState.nightLight
            subtitle: active ? "Until " + Qt.formatTime(new Date(QuickState.nightLightUntil), "HH:mm") : "Off"
            onClicked: QuickState.setNightLight(!active)
        }
        QsTile {
            visible: System.radios > 0
            icon: "plane"
            title: "Airplane Mode"
            active: System.airplane
            subtitle: active ? "On" : "Off"
            onClicked: System.setAirplane(!active)
        }
        QsTile {
            visible: PowerProfiles !== null
            icon: PowerProfiles.profile === PowerProfile.PowerSaver ? "leaf" : "zap"
            title: "Power Mode"
            active: PowerProfiles.profile !== PowerProfile.Balanced
            subtitle: QS.profileName(PowerProfiles.profile)
            onClicked: PowerProfiles.profile = QS.nextProfile(PowerProfiles.profile, PowerProfiles.hasPerformanceProfile)
        }
    }
    Grid {
        visible: root.powerMenu
        columns: 2
        spacing: 8
        QsTile {
            icon: "moon"; title: "Suspend"
            onClicked: { ShellState.closePanel(); ShellState.lock(); Quickshell.execDetached(["systemctl", "suspend"]) }
        }
        QsTile {
            icon: "logOut"; title: "Log Out"
            onClicked: Compositor.quit()
        }
        QsTile {
            icon: "restart"; title: "Restart"
            onClicked: Quickshell.execDetached(["systemctl", "reboot"])
        }
        QsTile {
            icon: "power"; title: "Power Off"; active: true
            onClicked: Quickshell.execDetached(["systemctl", "poweroff"])
        }
    }

    // Volume and brightness.
    Column {
        width: parent.width
        spacing: 8
        QsSlider {
            label: "Volume"
            visible: root.sink !== null
            icon: root.sink?.audio?.muted ? "volumeMuted" : "volume"
            dimmed: root.sink?.audio?.muted ?? false
            value: root.sink?.audio?.volume ?? 0
            onMoved: v => { root.sink.audio.volume = v; if (v > 0) root.sink.audio.muted = false }
            onIconClicked: root.sink.audio.muted = !root.sink.audio.muted
        }
        QsSlider {
            label: "Brightness"
            visible: System.brightness >= 0
            icon: "sun"
            minimum: 0.01
            value: System.brightness / 100
            onMoved: v => System.setBrightness(v * 100)
        }
    }

    // Battery, and the way into Settings.
    Item {
        width: parent.width
        height: 18
        Row {
            visible: root.hasBattery
            anchors.verticalCenter: parent.verticalCenter
            spacing: 6
            Icon { anchors.verticalCenter: parent.verticalCenter; name: root.batteryState === "charging" ? "zap" : "battery"; size: 16; color: Theme.muted }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: QS.batteryLine(root.percent, root.batteryState, root.battery?.timeToEmpty ?? 0, root.battery?.timeToFull ?? 0)
                color: Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(11.5) }
            }
        }
        Item {
            Accessible.role: Accessible.Link
            Accessible.name: "All settings"
            Accessible.onPressAction: ShellState.openSettings("")
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            width: allRow.implicitWidth
            height: allRow.implicitHeight
            Row {
                id: allRow
                spacing: 2
                Text {
                    text: "All settings"
                    color: allMouse.containsMouse ? Theme.text : Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(11.5); weight: Font.Medium }
                }
                Icon { anchors.verticalCenter: parent.verticalCenter; name: "chevR"; size: 13; color: allMouse.containsMouse ? Theme.text : Theme.muted }
            }
            MouseArea {
                id: allMouse
                anchors.fill: parent
                anchors.margins: -6
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: ShellState.openSettings("")
            }
        }
    }
}
