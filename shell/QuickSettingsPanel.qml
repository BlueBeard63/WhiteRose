// Quick settings (Figma: "Desktop - Quick Settings"): hangs from the right end
// of the bar, joined to it by an ear on the left. Opened from the status
// cluster or SUPER+S. Wi-Fi, Bluetooth, volume and battery come from
// QuickShell's services (NetworkManager, BlueZ, PipeWire, UPower);
// brightness and airplane mode from whiterosed; Night Light from the
// compositor; Do Not Disturb belongs to the shell.
import QtQuick
import Quickshell
import Quickshell.Bluetooth
import Quickshell.Io
import Quickshell.Networking
import Quickshell.Services.Pipewire
import Quickshell.Services.UPower
import Quickshell.Widgets
import "QuickSettings.js" as QS

Item {
    id: root

    readonly property bool open: ShellState.panel === "quick"
    property bool powerMenu: false
    onOpenChanged: {
        powerMenu = false;
        if (open) uptimeFile.reload();
    }

    // The column plus its top and bottom padding.
    readonly property real targetHeight: open ? content.implicitHeight + 16 + 20 : 0
    // Room the bar window must leave below the bar (grows at once, shrinks
    // after the closing animation), as for the island.
    property real reserve: 0
    onTargetHeightChanged: { if (targetHeight >= reserve) reserve = targetHeight; else shrink.restart() }
    Timer { id: shrink; interval: 320; onTriggered: root.reserve = root.targetHeight }

    width: shape.width
    height: shape.height
    visible: shape.bodyHeight > 0.5

    PanelShape {
        id: shape
        bodyWidth: 360
        bodyHeight: root.targetHeight
        rightEar: false
        earWidth: 49
        earHeight: Math.min(55, bodyHeight)
        bottomLeftRadius: 32
        bottomRightRadius: 0
        Behavior on bodyHeight { NumberAnimation { duration: 280; easing.type: Easing.OutCubic } }
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
    readonly property string name: QS.displayName(passwd.text(), user)
    FileView { id: uptimeFile; path: "/proc/uptime"; blockLoading: true }
    readonly property string uptime: QS.uptime(parseFloat(uptimeFile.text()) || 0)
    // AccountsService's picture (what GNOME's settings sets), else ~/.face.
    readonly property var avatars: ["/var/lib/AccountsService/icons/" + user, Quickshell.env("HOME") + "/.face"]

    // ---- keyboard: Esc closes, clicking elsewhere closes ----------------------
    FocusScope {
        id: keys
        anchors.fill: parent
        focus: root.open
        property bool focusSeen: false
        Keys.onEscapePressed: root.powerMenu ? root.powerMenu = false : ShellState.closePanel()
        onActiveFocusChanged: {
            if (activeFocus) focusSeen = true;
            else if (root.open && focusSeen) ShellState.closePanel();
        }

        Column {
            id: content
            x: shape.bodyX + 18
            y: 16
            width: 324
            spacing: 12
            opacity: root.open && !settle.running ? 1 : 0
            visible: opacity > 0
            Behavior on opacity { NumberAnimation { duration: 110 } }

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
                            text: QS.initials(root.name)
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
                            text: root.name
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
                        width: 30; height: 30; icon: "power"
                        iconColor: root.powerMenu ? Theme.bg : Theme.rose
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
                    visible: root.sink !== null
                    icon: root.sink?.audio?.muted ? "volumeMuted" : "volume"
                    dimmed: root.sink?.audio?.muted ?? false
                    value: root.sink?.audio?.volume ?? 0
                    onMoved: v => { root.sink.audio.volume = v; if (v > 0) root.sink.audio.muted = false }
                    onIconClicked: root.sink.audio.muted = !root.sink.audio.muted
                }
                QsSlider {
                    visible: System.brightness >= 0
                    icon: "sun"
                    minimum: 0.01
                    value: System.brightness / 100
                    onMoved: v => System.setBrightness(v * 100)
                }
            }

            // Battery.
            Row {
                visible: root.hasBattery
                spacing: 6
                Icon { anchors.verticalCenter: parent.verticalCenter; name: root.batteryState === "charging" ? "zap" : "battery"; size: 16; color: Theme.muted }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: QS.batteryLine(root.percent, root.batteryState, root.battery?.timeToEmpty ?? 0, root.battery?.timeToFull ?? 0)
                    color: Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(11.5) }
                }
            }
        }
    }

    // Content appears once the outline has (almost) finished growing.
    Timer { id: settle; interval: 200 }
    Connections {
        target: root
        function onOpenChanged() { if (root.open) settle.restart() }
    }
}
