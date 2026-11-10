// Enterprise (802.1X) Wi-Fi sign-in: university and work networks (eduroam
// and the like) where you sign in with your own username and password, or
// a certificate. Used when joining one and in a saved network's Security
// tab. `eap` is what whiterosed sends (netconf.EAP); `value` is the edited
// form, as whiterosed takes it (netconf.EAPChange). Secrets left empty keep
// the saved ones.
import QtQuick
import QtQuick.Dialogs
import Quickshell
import qs
import qs.settings

Column {
    id: form
    width: parent.width
    spacing: 10

    property var eap: null
    // Set when anything is edited.
    property var value: null
    // Joining: the password is needed (nothing saved yet).
    property bool joining: false

    property string method: "peap"
    property string phase2: "mschapv2"
    property string identity: ""
    property string anonymous: ""
    // file | system | none
    property string ca: "system"
    property string caCert: ""
    property string domain: ""
    // "" automatic, "0", "1"
    property string peapVersion: ""
    property string clientCert: ""
    property string privateKey: ""
    property string password: ""
    property string keyPassword: ""

    readonly property var phase2s: ({
        peap: [{ label: "MSCHAPv2", value: "mschapv2" }, { label: "GTC", value: "gtc" }, { label: "MD5", value: "md5" }],
        ttls: [{ label: "PAP", value: "pap" }, { label: "MSCHAPv2", value: "mschapv2" }, { label: "MSCHAP", value: "mschap" }, { label: "CHAP", value: "chap" }, { label: "GTC", value: "gtc" }],
    })
    // Enough filled in to save or connect.
    readonly property bool complete: identity.trim() !== ""
        && (method !== "tls" || (clientCert !== "" && privateKey !== ""))
        && (ca !== "file" || caCert !== "")
        && (!joining || method === "tls" || password !== "")

    function load(e) {
        e = e ?? {};
        form.method = e.method || "peap";
        form.phase2 = e.phase2 || (form.method === "ttls" ? "pap" : "mschapv2");
        form.identity = e.identity ?? "";
        form.anonymous = e.anonymous ?? "";
        form.caCert = e.ca_cert ?? "";
        form.ca = e.ca_cert ? "file" : (e.system_ca || !e.method) ? "system" : "none";
        form.domain = e.domain ?? "";
        form.peapVersion = e.peap_version ?? "";
        form.clientCert = e.client_cert ?? "";
        form.privateKey = e.private_key ?? "";
        form.password = "";
        form.keyPassword = "";
        form.value = null;
    }
    onEapChanged: load(eap)
    Component.onCompleted: load(eap)

    // The edited form, as whiterosed takes it.
    function build() {
        const v = {
            method: form.method,
            phase2: form.method === "peap" || form.method === "ttls" ? form.phase2 : "",
            identity: form.identity.trim(),
            anonymous: form.anonymous.trim(),
            ca_cert: form.ca === "file" ? form.caCert : "",
            system_ca: form.ca === "system",
            domain: form.domain.trim(),
            peap_version: form.method === "peap" ? form.peapVersion : "",
            client_cert: form.method === "tls" ? form.clientCert : "",
            private_key: form.method === "tls" ? form.privateKey : "",
        };
        if (form.password !== "") v.password = form.password;
        if (form.keyPassword !== "") v.key_password = form.keyPassword;
        return v;
    }
    function touch() { form.value = form.build() }

    Card {
        SettingRow {
            divider: false
            title: "Sign-in method"
            subtitle: "PEAP suits most university and work networks (eduroam)"
            Choice {
                value: form.method
                options: [{ label: "PEAP", value: "peap" }, { label: "TTLS", value: "ttls" }, { label: "TLS (certificate)", value: "tls" }, { label: "PWD", value: "pwd" }]
                onPicked: v => {
                    form.method = v;
                    if (form.phase2s[v] && !form.phase2s[v].some(o => o.value === form.phase2)) form.phase2 = form.phase2s[v][0].value;
                    form.touch();
                }
            }
        }
        SettingRow {
            visible: form.method === "peap"
            title: "PEAP version"
            Choice {
                value: form.peapVersion
                options: [{ label: "Automatic", value: "" }, { label: "Version 0", value: "0" }, { label: "Version 1", value: "1" }]
                onPicked: v => { form.peapVersion = v; form.touch() }
            }
        }
        SettingRow {
            visible: form.method === "peap" || form.method === "ttls"
            title: "Inner sign-in"
            Choice {
                value: form.phase2
                options: form.phase2s[form.method] ?? []
                onPicked: v => { form.phase2 = v; form.touch() }
            }
        }
        SettingRow {
            title: "Username"
            subtitle: "Often your email or student/staff ID"
            Field {
                implicitWidth: 240
                text: form.identity
                placeholder: "name@example.ac.uk"
                onTextChanged: if (text !== form.identity) { form.identity = text; form.touch() }
            }
        }
        SettingRow {
            visible: form.method !== "tls"
            title: "Password"
            subtitle: form.joining ? "" : form.eap?.has_password ? "Saved: type to change it" : "Not saved: asked when connecting"
            Field {
                implicitWidth: 240
                secret: true
                text: form.password
                placeholder: form.joining ? "Password" : "Unchanged"
                onTextChanged: if (text !== form.password) { form.password = text; form.touch() }
            }
        }
        SettingRow {
            visible: form.method === "tls"
            title: "Your certificate"
            Text { anchors.verticalCenter: parent.verticalCenter; width: 170; elide: Text.ElideMiddle; horizontalAlignment: Text.AlignRight; text: form.clientCert ? form.clientCert.split("/").pop() : "None"; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
            Button { text: "Choose…"; onClicked: { picker.target = "clientCert"; picker.open() } }
        }
        SettingRow {
            visible: form.method === "tls"
            title: "Private key"
            Text { anchors.verticalCenter: parent.verticalCenter; width: 170; elide: Text.ElideMiddle; horizontalAlignment: Text.AlignRight; text: form.privateKey ? form.privateKey.split("/").pop() : "None"; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
            Button { text: "Choose…"; onClicked: { picker.target = "privateKey"; picker.open() } }
        }
        SettingRow {
            visible: form.method === "tls"
            title: "Key password"
            Field {
                implicitWidth: 240
                secret: true
                text: form.keyPassword
                placeholder: form.joining ? "If the key has one" : "Unchanged"
                onTextChanged: if (text !== form.keyPassword) { form.keyPassword = text; form.touch() }
            }
        }
    }

    Section { text: "Checking the network" }
    Card {
        SettingRow {
            divider: false
            title: "CA certificate"
            subtitle: form.ca === "none" ? "Not checked: anyone could pretend to be this network and catch your password"
                : form.ca === "system" ? "Checked against the certificates this computer trusts" : ""
            Choice {
                value: form.ca
                options: [{ label: "This computer's", value: "system" }, { label: "From a file", value: "file" }, { label: "Don't check", value: "none" }]
                onPicked: v => { form.ca = v; if (v === "file" && !form.caCert) { picker.target = "caCert"; picker.open() } form.touch() }
            }
        }
        SettingRow {
            visible: form.ca === "file"
            title: "File"
            Text { anchors.verticalCenter: parent.verticalCenter; width: 200; elide: Text.ElideMiddle; horizontalAlignment: Text.AlignRight; text: form.caCert ? form.caCert.split("/").pop() : "None chosen"; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
            Button { text: "Choose…"; onClicked: { picker.target = "caCert"; picker.open() } }
        }
        SettingRow {
            title: "Domain"
            subtitle: "The network's server name must end with this (your IT pages say which)"
            Field {
                implicitWidth: 200
                text: form.domain
                placeholder: "example.ac.uk"
                onTextChanged: if (text !== form.domain) { form.domain = text; form.touch() }
            }
        }
        SettingRow {
            visible: form.method === "peap" || form.method === "ttls"
            title: "Anonymous identity"
            subtitle: "Sent before the secure link is up (optional)"
            Field {
                implicitWidth: 200
                text: form.anonymous
                placeholder: "anonymous@example.ac.uk"
                onTextChanged: if (text !== form.anonymous) { form.anonymous = text; form.touch() }
            }
        }
    }

    FileDialog {
        id: picker
        property string target: "caCert"
        title: target === "caCert" ? "Choose the CA certificate" : target === "clientCert" ? "Choose your certificate" : "Choose your private key"
        currentFolder: "file://" + (Quickshell.env("HOME") ?? "")
        nameFilters: ["Certificates and keys (*.pem *.crt *.cer *.der *.key *.p12 *.pfx)", "All files (*)"]
        onAccepted: {
            form[target] = decodeURIComponent(String(selectedFile).replace(/^file:\/\//, ""));
            form.touch();
        }
    }
}
