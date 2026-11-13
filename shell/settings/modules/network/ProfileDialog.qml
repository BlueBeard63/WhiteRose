// A saved network's settings, as its own page with ‹ Network (Figma:
// "Settings - Network › Home-5G · Details / Security / IPv4"): read and saved through whiterosed, which converts to
// NetworkManager's exact types and keeps the stored password. Only the
// fields you change are sent.
import QtQuick
import QtQuick.Controls
import Quickshell
import qs
import qs.settings

Dialog {
    id: dlg

    property string uuid
    property string kind: "wifi"   // wifi | ethernet
    property var profile: null     // as whiterosed sends it
    property var details: null
    property string tab: "details"
    property string error: ""
    property bool saving: false

    // Edited copies (null = untouched).
    property var edit: ({})
    function val(key, fallback) { return edit[key] !== undefined ? edit[key] : fallback }
    function change(key, value) { const e = Object.assign({}, edit); e[key] = value; edit = e }

    function show(uuid, name, kind) {
        dlg.uuid = uuid; dlg.kind = kind; dlg.title = name;
        dlg.profile = null; dlg.details = null; dlg.edit = {}; dlg.error = ""; dlg.tab = "details";
        dlg.open();
        Daemon.request({ op: "network_profile", uuid: uuid }, (v, err) => {
            if (err) { dlg.error = err; return; }
            dlg.profile = v.profile;
            dlg.details = v.details;
        });
    }

    readonly property var p: profile ?? { ipv4: { addresses: [], dns: [] }, ipv6: { addresses: [], dns: [] } }
    subtitle: !profile ? "Loading…"
        : (details?.active ? "Connected" : "Saved") + (kind === "wifi" ? " · " + securityLabel(p.security) : "")
    icon: kind === "wifi" ? "wifi" : "ethernet"

    function securityLabel(s) {
        return ({ "sae": "WPA3 Personal", "wpa-psk": "WPA2 / WPA3 Personal", "wpa-eap": "Enterprise", "owe": "Enhanced Open", "none": "Open" })[s] ?? s
    }

    // IP config being edited for a family ("ipv4" | "ipv6").
    function ip(family) { return val(family, p[family]) }
    function changeIp(family, field, value) {
        const cur = Object.assign({}, ip(family));
        cur[field] = value;
        change(family, cur);
    }
    function list(text) { return text.split(/[\s,]+/).map(s => s.trim()).filter(s => s) }

    function save() {
        const ch = {};
        for (const k of ["name", "autoconnect", "metered", "security", "mac", "pmf", "ipv4", "ipv6"])
            if (edit[k] !== undefined) ch[k] = edit[k];
        if ((edit.password ?? "") !== "") ch.password = edit.password;
        if (eapForm.value) ch.eap = eapForm.value;
        if (Object.keys(ch).length === 0) { dlg.close(); return; }
        dlg.saving = true;
        dlg.error = "";
        Daemon.request({ op: "network_update", uuid: dlg.uuid, changes: ch }, (v, err) => {
            dlg.saving = false;
            if (err) dlg.error = err;
            else dlg.close();
        });
    }

    // ---- tabs ------------------------------------------------------------------
    Row {
        spacing: 18
        Repeater {
            model: [["details", "Details"], ["security", "Security"], ["ipv4", "IPv4"], ["ipv6", "IPv6"], ["dns", "DNS"], ["advanced", "Advanced"]]
                .filter(t => t[0] !== "security" || dlg.kind === "wifi")
            delegate: Column {
                required property var modelData
                readonly property bool on: dlg.tab === modelData[0]
                spacing: 6
                Accessible.role: Accessible.PageTab
                Accessible.name: modelData[1]
                Accessible.selected: on
                Accessible.onPressAction: dlg.tab = modelData[0]
                Text {
                    text: modelData[1]
                    color: parent.on ? Theme.text : Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(12.5); weight: parent.on ? Font.Medium : Font.Normal }
                    MouseArea { anchors.fill: parent; anchors.margins: -6; cursorShape: Qt.PointingHandCursor; onClicked: dlg.tab = modelData[0] }
                }
                Rectangle { width: parent.width; height: 2; radius: 1; color: parent.on ? Theme.rose : "transparent" }
            }
        }
    }

    // ---- Details -------------------------------------------------------------------
    Card {
        visible: dlg.tab === "details" && dlg.profile !== null
        color: Theme.surface1
        Repeater {
            model: dlg.details?.active ? [
                ["Interface", dlg.details.device],
                ["Hardware address", dlg.details.mac],
                ["Link speed", dlg.details.speed ? dlg.details.speed + " Mb/s" : "—"],
                ["IPv4 address", (dlg.details.ipv4 ?? []).join(", ") || "—"],
                ["Router", dlg.details.gateway4 || "—"],
                ["DNS", (dlg.details.dns ?? []).join(", ") || "—"],
                ["IPv6 address", (dlg.details.ipv6 ?? []).join(", ") || "—"],
            ] : [["Status", "Not connected right now"]]
            delegate: SettingRow {
                required property var modelData
                required property int index
                divider: index > 0
                title: modelData[0]
                // (Several addresses: the first, the rest on hover.)
                Text {
                    id: val
                    anchors.verticalCenter: parent.verticalCenter
                    width: Math.min(implicitWidth, 420)
                    elide: Text.ElideRight
                    text: modelData[1]
                    color: Theme.muted
                    font { family: Theme.mono; pointSize: Theme.pt(11.5) }
                    MouseArea { id: valMouse; anchors.fill: parent; hoverEnabled: true; acceptedButtons: Qt.NoButton }
                    ToolTip.visible: valMouse.containsMouse && val.truncated
                    ToolTip.text: modelData[1].split(", ").join("\n")
                }
            }
        }
    }

    // ---- Security -------------------------------------------------------------------
    Column {
        visible: dlg.tab === "security" && dlg.profile !== null
        width: parent.width
        spacing: 12
        Card {
            visible: dlg.p.security !== "wpa-eap"
            SettingRow {
                id: secRow
                divider: false
                readonly property var types: [
                    { value: "sae", label: "WPA3 Personal", hint: "recommended", about: "Strongest option for home networks" },
                    { value: "wpa-psk", label: "WPA2 / WPA3 Personal", about: "Works with older devices" },
                    { value: "owe", label: "Enhanced Open", about: "Encrypted, no password" },
                    { value: "none", label: "Open", hint: "not secure", about: "No encryption: anyone nearby can read traffic" },
                ]
                readonly property string current: dlg.val("security", dlg.p.security)
                title: "Security type"
                subtitle: (types.find(t => t.value === current) ?? { about: "" }).about
                subtitleColor: current === "none" ? Theme.rose : Theme.muted
                Choice {
                    value: secRow.current
                    options: secRow.types
                    onPicked: v => dlg.change("security", v)
                }
            }
        }
        Text {
            visible: dlg.p.security === "wpa-eap"
            width: parent.width; wrapMode: Text.Wrap
            text: "Enterprise (802.1X): you sign in with your own account, not a shared password."
            color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(12) }
        }
        EnterpriseForm {
            id: eapForm
            visible: dlg.p.security === "wpa-eap"
            eap: dlg.profile?.eap ?? null
        }
        Column {
            visible: ["sae", "wpa-psk"].includes(dlg.val("security", dlg.p.security))
            width: parent.width
            spacing: 6
            Section { text: "Password" }
            Field {
                width: parent.width
                secret: true
                placeholder: "Unchanged (type to set a new one)"
                text: dlg.edit.password ?? ""
                onTextChanged: if (text !== (dlg.edit.password ?? "")) dlg.change("password", text)
                error: text !== "" && (text.length < 8 || text.length > 63)
            }
        }
        Card {
            SettingRow {
                divider: false
                title: "MAC address"
                subtitle: "What this computer shows the network"
                Choice {
                    value: dlg.val("mac", dlg.p.mac)
                    options: [
                        { label: "Default", value: "default" },
                        { label: "Stable random per network", value: "stable" },
                        { label: "New random each time", value: "random" },
                        { label: "Real hardware address", value: "permanent" },
                    ]
                    onPicked: v => dlg.change("mac", v)
                }
            }
            SettingRow {
                visible: dlg.val("security", dlg.p.security) !== "none"
                title: "Management frame protection"
                subtitle: "Blocks forged disconnect attacks (802.11w)"
                Choice {
                    value: dlg.val("pmf", dlg.p.pmf)
                    options: [{ label: "Default", value: "default" }, { label: "Optional", value: "optional" }, { label: "Required", value: "required" }, { label: "Off", value: "disable" }]
                    onPicked: v => dlg.change("pmf", v)
                }
            }
        }
    }

    // ---- IPv4 / IPv6 ---------------------------------------------------------------
    component IpTab: Column {
        id: ipTab
        required property string family
        readonly property var cfg: dlg.ip(family)
        readonly property var first: (cfg.addresses ?? [])[0] ?? { address: "", prefix: family === "ipv4" ? 24 : 64 }
        visible: dlg.tab === family && dlg.profile !== null
        width: parent.width
        spacing: 12
        Segmented {
            anchors.verticalCenter: undefined
            value: ipTab.cfg.method === "dhcp" ? "auto" : ipTab.cfg.method
            options: ipTab.family === "ipv4"
                ? [{ label: "Automatic (DHCP)", value: "auto" }, { label: "Manual", value: "manual" }, { label: "Link-local only", value: "link-local" }, { label: "Off", value: "disabled" }]
                : [{ label: "Automatic", value: "auto" }, { label: "Manual", value: "manual" }, { label: "Ignore", value: "ignore" }, { label: "Off", value: "disabled" }]
            onPicked: v => {
                dlg.changeIp(ipTab.family, "method", v);
                if (v === "manual" && (ipTab.cfg.addresses ?? []).length === 0) dlg.changeIp(ipTab.family, "addresses", [ipTab.first]);
            }
        }
        Card {
            visible: ipTab.cfg.method === "manual"
            SettingRow {
                divider: false; title: "Address"
                Field {
                    implicitWidth: 200; mono: true; text: ipTab.first.address
                    placeholder: ipTab.family === "ipv4" ? "192.168.1.42" : "2001:db8::42"
                    onTextChanged: if (text !== ipTab.first.address) dlg.changeIp(ipTab.family, "addresses", [{ address: text, prefix: ipTab.first.prefix }])
                }
                Text { text: "/"; color: Theme.muted; anchors.verticalCenter: parent.verticalCenter; font { family: Theme.mono; pointSize: Theme.pt(13) } }
                Field {
                    implicitWidth: 56; mono: true; text: String(ipTab.first.prefix)
                    onTextChanged: { const n = parseInt(text); if (n && n !== ipTab.first.prefix) dlg.changeIp(ipTab.family, "addresses", [{ address: ipTab.first.address, prefix: n }]) }
                }
            }
            SettingRow {
                title: "Router"
                Field {
                    implicitWidth: 270; mono: true; text: ipTab.cfg.gateway ?? ""
                    placeholder: ipTab.family === "ipv4" ? "192.168.1.1" : "fe80::1"
                    onTextChanged: if (text !== (ipTab.cfg.gateway ?? "")) dlg.changeIp(ipTab.family, "gateway", text)
                }
            }
        }
    }
    IpTab { family: "ipv4" }
    IpTab { family: "ipv6" }

    // ---- DNS ---------------------------------------------------------------------------
    Card {
        visible: dlg.tab === "dns" && dlg.profile !== null
        SettingRow {
            divider: false; title: "IPv4 DNS servers"; subtitle: "Separate with commas"
            Field {
                implicitWidth: 240; mono: true; text: (dlg.ip("ipv4").dns ?? []).join(", ")
                placeholder: "From the network"
                onTextChanged: if (text !== (dlg.ip("ipv4").dns ?? []).join(", ")) dlg.changeIp("ipv4", "dns", dlg.list(text))
            }
        }
        SettingRow {
            title: "IPv6 DNS servers"
            Field {
                implicitWidth: 240; mono: true; text: (dlg.ip("ipv6").dns ?? []).join(", ")
                placeholder: "From the network"
                onTextChanged: if (text !== (dlg.ip("ipv6").dns ?? []).join(", ")) dlg.changeIp("ipv6", "dns", dlg.list(text))
            }
        }
        SettingRow {
            title: "Use only these servers"
            subtitle: "Ignore the ones the network suggests"
            Toggle {
                checked: dlg.ip("ipv4").ignore_auto_dns ?? false
                onToggled: on => { dlg.changeIp("ipv4", "ignore_auto_dns", on); dlg.changeIp("ipv6", "ignore_auto_dns", on) }
            }
        }
    }

    // ---- Advanced --------------------------------------------------------------------
    Card {
        visible: dlg.tab === "advanced" && dlg.profile !== null
        SettingRow {
            divider: false; title: "Name"
            Field {
                implicitWidth: 220; text: dlg.p.name ?? ""
                onTextChanged: if (text !== (dlg.p.name ?? "")) dlg.change("name", text)
            }
        }
        SettingRow {
            title: "Connect automatically"
            Toggle { checked: dlg.val("autoconnect", dlg.p.autoconnect) ?? true; onToggled: on => dlg.change("autoconnect", on) }
        }
        SettingRow {
            title: "Metered connection"
            subtitle: "Apps hold back big downloads and updates"
            Choice {
                value: dlg.val("metered", dlg.p.metered)
                options: [{ label: "Automatic", value: "auto" }, { label: "Yes", value: "yes" }, { label: "No", value: "no" }]
                onPicked: v => dlg.change("metered", v)
            }
        }
    }

    Text {
        visible: dlg.error !== ""
        width: parent.width; wrapMode: Text.Wrap
        text: dlg.error
        color: Theme.rose
        font { family: Theme.font; pointSize: Theme.pt(12) }
    }

    footer: [
        Button { id: cancelBtn; text: "Cancel"; onClicked: dlg.close() },
        Button { id: saveBtn; text: dlg.saving ? "Saving…" : "Save"; kind: "primary"; enabled: dlg.profile !== null && !dlg.saving; onClicked: dlg.save() }
    ]
}
