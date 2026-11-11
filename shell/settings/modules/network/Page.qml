// Network (Figma: "Settings - Network"): Wi-Fi on/off, the network you're
// on, others in range (connect, with a password prompt when needed), and
// Ethernet. "Settings" opens the profile dialog. Scanning runs only while
// this page is open.
import QtQuick
import Quickshell
import Quickshell.Networking
import qs
import qs.settings

Column {
    id: page
    spacing: 16

    readonly property var wifi: Networking.devices.values.find(d => d.type === DeviceType.Wifi) ?? null
    readonly property var wired: Networking.devices.values.filter(d => d.type === DeviceType.Wired)
    readonly property var networks: (wifi?.networks.values ?? []).slice()
        .sort((a, b) => (b.connected - a.connected) || (b.known - a.known) || (b.signalStrength - a.signalStrength))
    readonly property var current: networks.find(n => n.connected) ?? null
    readonly property var others: networks.filter(n => !n.connected && n.name !== "")

    Component.onCompleted: if (wifi) wifi.scannerEnabled = true
    Component.onDestruction: if (wifi) wifi.scannerEnabled = false

    function secure(n) { return n.security !== WifiSecurityType.Open && n.security !== WifiSecurityType.Owe }
    function securityName(n) {
        return ({
            [WifiSecurityType.Sae]: "WPA3", [WifiSecurityType.Wpa3SuiteB192]: "WPA3",
            [WifiSecurityType.Wpa2Psk]: "WPA2", [WifiSecurityType.WpaPsk]: "WPA",
            [WifiSecurityType.Wpa2Eap]: "Enterprise", [WifiSecurityType.WpaEap]: "Enterprise",
            [WifiSecurityType.StaticWep]: "WEP", [WifiSecurityType.DynamicWep]: "WEP",
            [WifiSecurityType.Owe]: "OWE", [WifiSecurityType.Open]: "Open",
        })[n.security] ?? ""
    }
    // Joining a network that needs a password or a sign-in: asked under its
    // row (the network's name, "" for none). Clicking it again closes it.
    property string joining: ""
    property string joinError: ""
    property bool joinBusy: false
    function join(n) {
        if (n.known || !secure(n)) { page.joining = ""; n.connect(); return }
        page.joinError = ""; page.joinBusy = false;
        page.joining = page.joining === n.name ? "" : n.name;
    }

    PageHeader { title: "Network"; subtitle: "Wi-Fi and Ethernet, managed through NetworkManager." }

    // ---- Wi-Fi ---------------------------------------------------------------
    Card {
        visible: page.wifi !== null
        Item {
            width: parent.width; height: 64
            Rectangle {
                id: wifiTile
                x: 14; anchors.verticalCenter: parent.verticalCenter
                width: 36; height: 36; radius: 10
                color: Networking.wifiEnabled ? Theme.rose : Theme.surface2
                Icon { anchors.centerIn: parent; name: Networking.wifiEnabled ? "wifi" : "wifiOff"; size: 17; color: Networking.wifiEnabled ? Theme.onAccent : Theme.muted }
            }
            Column {
                anchors { left: wifiTile.right; leftMargin: 12; verticalCenter: parent.verticalCenter }
                Text { text: "Wi-Fi"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold } }
                Text {
                    text: !Networking.wifiHardwareEnabled ? "Turned off by a hardware switch"
                        : (page.wifi?.name ?? "") + (Networking.wifiEnabled ? " · scanning while this page is open" : "")
                    color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) }
                }
            }
            Toggle {
                anchors { right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
                checked: Networking.wifiEnabled
                enabled: Networking.wifiHardwareEnabled
                onToggled: on => Networking.wifiEnabled = on
            }
        }

        // The network you're on.
        Rectangle {
            visible: page.current !== null && Networking.wifiEnabled
            width: parent.width; height: 60
            color: Theme.surface2
            SignalBars { id: curBars; x: 16; strength: page.current?.signalStrength ?? 0 }
            Column {
                anchors { left: curBars.right; leftMargin: 14; verticalCenter: parent.verticalCenter }
                spacing: 2
                Row {
                    spacing: 6
                    Text { text: page.current?.name ?? ""; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.DemiBold } }
                    Badge { text: page.current ? page.securityName(page.current) : ""; visible: text !== "" }
                }
                Row {
                    spacing: 5
                    Rectangle { width: 5; height: 5; radius: 3; color: Theme.rose; anchors.verticalCenter: parent.verticalCenter }
                    Text { text: "Connected"; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
                }
            }
            Row {
                anchors { right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
                spacing: 8
                Button { text: "Disconnect"; kind: "ghost"; onClicked: page.current.disconnect() }
                Button {
                    text: "Settings"; icon: "gear"
                    visible: (page.current?.nmSettings ?? []).length > 0
                    onClicked: profileDialog.show(page.current.nmSettings[0].uuid, page.current.name, "wifi")
                }
            }
        }

        Item {
            visible: Networking.wifiEnabled && page.others.length > 0
            width: parent.width; height: 30
            Section { x: 14; anchors.bottom: parent.bottom; anchors.bottomMargin: 4; text: "Other networks" }
        }
        Repeater {
            model: Networking.wifiEnabled ? page.others : []
            delegate: Column {
                id: netItem
                required property var modelData
                width: parent.width
                readonly property bool asking: page.joining !== "" && page.joining === modelData.name
            Rectangle {
                id: net
                readonly property var n: netItem.modelData
                width: parent.width; height: 46
                color: netItem.asking ? Theme.tint(0.12) : netMouse.containsMouse ? Theme.surface2 : "transparent"
                SignalBars { id: bars; x: 16; strength: net.n.signalStrength; color: Theme.muted }
                Column {
                    anchors { left: bars.right; leftMargin: 14; verticalCenter: parent.verticalCenter }
                    spacing: 1
                    Row {
                        spacing: 6
                        Text { text: net.n.name; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(12.5) } }
                        Badge { visible: net.n.known; text: "saved" }
                    }
                    Text {
                        visible: text !== ""
                        text: net.n.stateChanging || net.n.state === ConnectionState.Connecting ? "Connecting…"
                            : !page.secure(net.n) ? "Open · not encrypted"
                            : page.securityName(net.n) === "Enterprise" ? "Enterprise" : ""
                        color: !page.secure(net.n) ? Theme.rose : Theme.muted
                        font { family: Theme.font; pointSize: Theme.pt(11) }
                    }
                }
                Row {
                    anchors { right: parent.right; rightMargin: 16; verticalCenter: parent.verticalCenter }
                    spacing: 12
                    Icon { visible: page.secure(net.n); name: "lock"; size: 13; color: Theme.muted; anchors.verticalCenter: parent.verticalCenter }
                    Icon {
                        visible: net.n.known
                        name: "trash"; size: 13; color: forgetMouse.containsMouse ? Theme.rose : Theme.faint
                        anchors.verticalCenter: parent.verticalCenter
                        MouseArea { id: forgetMouse; anchors.fill: parent; anchors.margins: -6; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: net.n.forget() }
                    }
                    Icon { name: netItem.asking ? "chevDown" : "chevR"; size: 13; color: Theme.faint; anchors.verticalCenter: parent.verticalCenter }
                }
                MouseArea {
                    id: netMouse
                    anchors.fill: parent
                    z: -1
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: page.join(net.n)
                }
            }
            // ---- asked right here: its password, or (Enterprise) your sign-in ----
            Loader {
                active: netItem.asking
                visible: active
                width: parent.width
                sourceComponent: page.securityName(netItem.modelData) === "Enterprise" ? enterpriseAsk : passwordAsk
                property var network: netItem.modelData
            }
            }
        }
        // Breathing room under the list (not under the connected row alone).
        Item { width: 1; height: 6; visible: Networking.wifiEnabled && page.others.length > 0 }
    }

    // ---- Ethernet ---------------------------------------------------------------
    Repeater {
        model: page.wired
        delegate: Card {
            id: eth
            required property var modelData
            readonly property var dev: modelData
            Item {
                width: parent.width; height: 64
                Rectangle {
                    id: ethTile
                    x: 14; anchors.verticalCenter: parent.verticalCenter
                    width: 36; height: 36; radius: 10
                    color: eth.dev.connected ? Theme.rose : Theme.surface2
                    Icon { anchors.centerIn: parent; name: "ethernet"; size: 17; color: eth.dev.connected ? Theme.onAccent : Theme.muted }
                }
                Column {
                    anchors { left: ethTile.right; leftMargin: 12; verticalCenter: parent.verticalCenter }
                    Text { text: "Ethernet"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold } }
                    Text {
                        text: eth.dev.connected ? "Connected" + (eth.dev.linkSpeed ? " · " + eth.dev.linkSpeed + " Mb/s" : "")
                            : eth.dev.hasLink ? "Cable plugged in" : "Cable unplugged"
                        color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) }
                    }
                }
                Button {
                    anchors { right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
                    text: "Settings"; icon: "gear"
                    visible: (eth.dev.network?.nmSettings ?? []).length > 0
                    onClicked: profileDialog.show(eth.dev.network.nmSettings[0].uuid, "Ethernet", "ethernet")
                }
            }
        }
    }

    // ---- joining, under the network's row -----------------------------------------------
    Component {
        id: passwordAsk
        Rectangle {
            id: pa
            readonly property var network: parent?.network ?? null
            height: 52
            color: Theme.tint(0.12)
            Keys.onEscapePressed: page.joining = ""
            Row {
                anchors { left: parent.left; leftMargin: 46; right: parent.right; rightMargin: 16; verticalCenter: parent.verticalCenter }
                spacing: 8
                Field {
                    id: pw
                    width: Math.min(360, parent.width - joinBtn.width - cancelJoin.width - 24)
                    height: 34
                    secret: true
                    placeholder: "Password for " + (pa.network?.name ?? "") + " (" + page.securityName(pa.network ?? {}) + ")"
                    onAccepted: if (joinBtn.enabled) joinBtn.clicked()
                    Keys.onEscapePressed: page.joining = ""
                    Component.onCompleted: Qt.callLater(() => pw.focusField())
                }
                Item { width: Math.max(0, parent.width - pw.width - joinBtn.width - cancelJoin.width - 24); height: 1 }
                Button { id: cancelJoin; text: "Cancel"; kind: "ghost"; onClicked: page.joining = "" }
                Button {
                    id: joinBtn; text: "Join"; kind: "primary"
                    enabled: pw.text.length >= 8
                    onClicked: { pa.network.connectWithPsk(pw.text); page.joining = "" }
                }
            }
        }
    }
    // Enterprise networks: your own sign-in instead of one password.
    Component {
        id: enterpriseAsk
        Rectangle {
            id: ea
            readonly property var network: parent?.network ?? null
            height: eaCol.implicitHeight + 24
            color: Theme.tint(0.12)
            Keys.onEscapePressed: page.joining = ""
            Column {
                id: eaCol
                x: 46; y: 8
                width: Math.min(560, parent.width - 62)
                spacing: 10
                Text { text: "Sign in with your account"; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
                EnterpriseForm { id: joinForm; joining: true; width: parent.width; Component.onCompleted: joinForm.load(null) }
                Text {
                    visible: page.joinError !== ""
                    width: parent.width; wrapMode: Text.Wrap
                    text: page.joinError
                    color: Theme.rose; font { family: Theme.font; pointSize: Theme.pt(11.5) }
                }
                Row {
                    spacing: 8
                    Button { anchors.verticalCenter: undefined; text: "Cancel"; kind: "ghost"; onClicked: page.joining = "" }
                    Button {
                        anchors.verticalCenter: undefined
                        text: page.joinBusy ? "Connecting…" : "Connect"; kind: "primary"
                        enabled: joinForm.complete && !page.joinBusy
                        onClicked: {
                            page.joinBusy = true;
                            page.joinError = "";
                            Daemon.request({ op: "network_join_enterprise", args: { ssid: ea.network.name, eap: joinForm.build() } }, (v, err) => {
                                page.joinBusy = false;
                                if (err) page.joinError = String(err);
                                else page.joining = "";
                            });
                        }
                    }
                }
            }
        }
    }
    ProfileDialog { id: profileDialog }
}
