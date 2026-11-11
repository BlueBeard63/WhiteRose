// The Sheet's Server tab: a network share (Windows/Samba, SSH, FTP, WebDAV
// or NFS) opened as a folder through GVfs (shares.rs). The password can be
// kept in the keyring (through whiterosed) and the server in the sidebar.
// A question from the server (an SSH host key not seen before, a
// certificate that can't be checked) is shown right here, and only trusted
// when you say so.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files

Column {
    id: root
    // Opened at this folder once connected.
    signal opened_(string path)
    signal changed()
    signal closed()

    property int job: -1
    property bool busy: false
    property string error: ""
    // A server's question waiting for an answer.
    property string question: ""
    property bool wrong: false
    // Hosts from ~/.ssh/config (ssh knows their user, port and key).
    property var sshHosts: []
    readonly property string scheme: (address.text.match(/^\s*([a-z]+):\/\//i)?.[1] ?? (address.text.trim().startsWith("\\\\") || address.text.trim() !== "" ? "smb" : "")).toLowerCase()
    readonly property bool typing: address.editing || user.editing || domain.editing || password.editing

    // Fresh, or for a saved server that needs its password.
    function show(server) {
        root.error = ""; root.question = ""; root.wrong = false; root.busy = false;
        address.text = server?.uri ?? "";
        user.text = server?.user ?? "";
        domain.text = server?.domain ?? "";
        password.text = "";
        remember.checked = true;
        keep.checked = true;
        root.sshHosts = JSON.parse(Ops.sshHosts());
        Qt.callLater(() => (server ? password : address).focusField());
    }
    function go(accept) {
        root.error = "";
        root.busy = true;
        root.job = Ops.connectServer(JSON.stringify({
            address: address.text, user: user.text.trim(), domain: domain.text.trim(), password: password.text,
            remember: remember.checked, save: keep.checked, accept: !!accept,
        }));
    }
    function cancel() {
        if (root.busy) { Ops.stopCleanup(root.job); root.job = -1; root.busy = false; return }
        root.question = "";
        root.closed();
    }
    // A kind picked: the address starts with its scheme.
    function useScheme(s) {
        const t = address.text.trim();
        const rest = t.replace(/^\s*[a-z]+:\/\//i, "").replace(/^\\\\/, "").replace(/\\/g, "/");
        address.text = s + "://" + rest;
        address.focusField();
    }
    Connections {
        target: Ops
        function onServerConnected(id, result) {
            if (id !== root.job) return;
            root.busy = false;
            const r = JSON.parse(result);
            if (r.result === "ok") {
                root.question = "";
                password.text = "";
                root.changed();
                root.opened_(r.path);
                root.closed();
            } else if (r.result === "password") {
                root.wrong = r.wrong;
                root.error = r.wrong ? "That password didn't work" : "This server needs a password";
                password.focusField();
            } else if (r.result === "question") {
                root.question = r.text;
            } else {
                root.error = r.error;
            }
        }
    }

    spacing: 12

    component Label: Text {
        color: Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(10.5) }
    }
    component SwitchRow: Item {
        id: sw
        property string text
        property alias checked: tg.checked
        width: parent.width; height: 30
        Text { anchors.verticalCenter: parent.verticalCenter; text: sw.text; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(12) } }
        Toggle { id: tg; anchors { right: parent.right; verticalCenter: parent.verticalCenter } onToggled: on => checked = on }
    }

    Text { text: "Connect to a server"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold } }
    Column {
        width: parent.width
        spacing: 4
        Label { text: "Address" }
        Field { id: address; width: parent.width; height: 34; mono: true; placeholder: "smb://nas/photos"; onAccepted: root.go(false); onTextChanged: root.error = "" }
    }
    // The kind of server (the address's scheme; picking one sets it).
    Item {
        width: parent.width; height: 32
        Text { anchors.verticalCenter: parent.verticalCenter; text: "Kind"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(12) } }
        Choice {
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            value: root.scheme === "dav" ? "davs" : root.scheme === "ftps" ? "ftp" : root.scheme
            options: [{ label: "Windows / Samba (SMB)", value: "smb" }, { label: "SSH (SFTP)", value: "sftp" }, { label: "FTP", value: "ftp" }, { label: "WebDAV", value: "davs" }, { label: "NFS", value: "nfs" }]
            onPicked: v => root.useScheme(v)
        }
    }
    Text {
        width: parent.width
        wrapMode: Text.Wrap
        text: "smb://server/share or \\\\server\\share · sftp://user@host · ftp://host · davs://host/path · nfs://host/path"
        color: Theme.faint
        font { family: Theme.font; pointSize: Theme.pt(10) }
    }
    // One pick for a host in the SSH config.
    Item {
        visible: root.sshHosts.length > 0 && !root.busy
        width: parent.width; height: 32
        Text { anchors.verticalCenter: parent.verticalCenter; text: "From SSH config"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(12) } }
        Choice {
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            value: root.sshHosts.find(h => address.text.trim() === "sftp://" + h) ?? ""
            options: [{ label: "Choose…", value: "" }].concat(root.sshHosts.map(h => ({ label: h, value: h })))
            onPicked: v => { if (!v) return; address.text = "sftp://" + v; user.text = ""; password.focusField() }
        }
    }
    Column {
        width: parent.width
        spacing: 4
        Label { text: "Username" }
        Field { id: user; width: parent.width; height: 34; placeholder: root.scheme === "sftp" ? "Yours there (or from the SSH config)" : "Empty: sign in as a guest"; onAccepted: root.go(false) }
    }
    Column {
        visible: root.scheme === "smb"
        width: parent.width
        spacing: 4
        Label { text: "Domain" }
        Field { id: domain; width: parent.width; height: 34; placeholder: "WORKGROUP (usually right)"; onAccepted: root.go(false) }
    }
    Column {
        width: parent.width
        spacing: 4
        Label { text: "Password" }
        Field { id: password; width: parent.width; height: 34; secret: true; error: root.wrong; placeholder: root.scheme === "sftp" ? "Not needed with an SSH key" : ""; onAccepted: root.go(false); onTextChanged: root.wrong = false }
    }
    Column {
        width: parent.width
        SwitchRow { id: remember; text: "Remember password"; checked: true }
        SwitchRow { id: keep; text: "Show in the sidebar"; checked: true }
    }

    // The server asks something (host key, certificate): answered here.
    Rectangle {
        visible: root.question !== ""
        width: parent.width
        height: qCol.implicitHeight + 24
        radius: 10
        color: Qt.rgba(0.89, 0.70, 0.25, 0.08)
        border { width: 1; color: Qt.rgba(0.89, 0.70, 0.25, 0.5) }
        Column {
            id: qCol
            x: 12; y: 12
            width: parent.width - 24
            spacing: 8
            Row {
                spacing: 7
                Icon { anchors.verticalCenter: parent.verticalCenter; name: "alert"; size: 13; color: "#E3B341" }
                Text { text: "The server asks"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(12); weight: Font.Medium } }
            }
            Text {
                width: parent.width
                wrapMode: Text.Wrap
                text: root.question + "\n\nOnly trust it if you know this is the right server (on a new server, or after it was set up again, this is normal)."
                color: Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(11) }
            }
            Row {
                spacing: 6
                Button { anchors.verticalCenter: undefined; text: "Don't connect"; kind: "ghost"; onClicked: { root.question = "" } }
                Button { anchors.verticalCenter: undefined; text: "Trust and connect"; kind: "danger"; enabled: !root.busy; onClicked: root.go(true) }
            }
        }
    }
    Text {
        visible: root.error !== "" || root.busy
        width: parent.width
        wrapMode: Text.Wrap
        text: root.busy ? "Connecting…" : root.error
        color: root.busy ? Theme.muted : Theme.rose
        font { family: Theme.font; pointSize: Theme.pt(11.5) }
    }
    Row {
        spacing: 8
        Button { anchors.verticalCenter: undefined; text: root.busy ? "Stop" : "Cancel"; kind: "ghost"; onClicked: root.cancel() }
        Button {
            visible: root.question === ""
            anchors.verticalCenter: undefined
            text: "Connect"; kind: "primary"
            enabled: !root.busy && address.text.trim() !== ""
            onClicked: root.go(false)
        }
    }
}
