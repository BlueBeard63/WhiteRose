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
    function join(n) {
        if (n.known || !secure(n)) n.connect();
        else if (securityName(n) === "Enterprise") { enterpriseDialog.network = n; enterpriseDialog.open(); }
        else { passwordDialog.network = n; passwordDialog.open(); }
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
            delegate: Rectangle {
                id: net
                required property var modelData
                readonly property var n: modelData
                width: parent.width; height: 46
                color: netMouse.containsMouse ? Theme.surface2 : "transparent"
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
                    Icon { name: "chevR"; size: 13; color: Theme.faint; anchors.verticalCenter: parent.verticalCenter }
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

    // ---- dialogs -------------------------------------------------------------------
    Dialog {
        id: passwordDialog
        property var network: null
        title: network?.name ?? ""
        subtitle: network ? page.securityName(network) + " · needs a password" : ""
        icon: "lock"
        width: 420
        onOpened: { pw.text = ""; pw.focusField() }
        Field { id: pw; width: parent.width; secret: true; placeholder: "Password"; onAccepted: connectBtn.clicked() }
        footer: [
            Item { width: parent.width - cancelBtn.width - connectBtn.width - 16; height: 1 },
            Button { id: cancelBtn; text: "Cancel"; onClicked: passwordDialog.close() },
            Button {
                id: connectBtn; text: "Connect"; kind: "primary"
                enabled: pw.text.length >= 8
                onClicked: { passwordDialog.network.connectWithPsk(pw.text); passwordDialog.close() }
            }
        ]
    }
    // Enterprise networks: your own sign-in instead of one password.
    Dialog {
        id: enterpriseDialog
        property var network: null
        property string error: ""
        property bool busy: false
        title: network?.name ?? ""
        subtitle: "Enterprise · sign in with your account"
        icon: "lock"
        width: 560
        onOpened: { error = ""; busy = false; joinForm.load(null) }
        EnterpriseForm { id: joinForm; joining: true }
        Text {
            visible: enterpriseDialog.error !== ""
            width: parent.width; wrapMode: Text.Wrap
            text: enterpriseDialog.error
            color: Theme.rose; font { family: Theme.font; pointSize: Theme.pt(11.5) }
        }
        footer: [
            Item { width: parent.width - eCancel.width - eConnect.width - 16; height: 1 },
            Button { id: eCancel; text: "Cancel"; onClicked: enterpriseDialog.close() },
            Button {
                id: eConnect; text: enterpriseDialog.busy ? "Connecting…" : "Connect"; kind: "primary"
                enabled: joinForm.complete && !enterpriseDialog.busy
                onClicked: {
                    enterpriseDialog.busy = true;
                    enterpriseDialog.error = "";
                    Daemon.request({ op: "network_join_enterprise", args: { ssid: enterpriseDialog.network.name, eap: joinForm.build() } }, (v, err) => {
                        enterpriseDialog.busy = false;
                        if (err) enterpriseDialog.error = String(err);
                        else enterpriseDialog.close();
                    });
                }
            }
        ]
    }
    ProfileDialog { id: profileDialog }
}
