// Connect to server: a network share (Windows/Samba, SSH, FTP, WebDAV or
// NFS) opened as a folder through GVfs (shares.rs). The password can be
// kept in the keyring (through whiterosed) and the server in the sidebar.
// A question from the server (an SSH host key not seen before, a
// certificate that can't be checked) is shown, and only trusted when you
// say so.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files

Popup {
    id: root
    // Opened at this folder once connected.
    signal opened_(string path)
    signal changed()

    property int job: -1
    property bool busy: false
    property string error: ""
    // A server's question waiting for an answer.
    property string question: ""
    property bool wrong: false
    readonly property string scheme: (address.text.match(/^\s*([a-z]+):\/\//i)?.[1] ?? (address.text.trim().startsWith("\\\\") || address.text.trim() !== "" ? "smb" : "")).toLowerCase()

    // Fresh, or for a saved server that needs its password.
    function show(server) {
        root.error = ""; root.question = ""; root.wrong = false; root.busy = false;
        address.text = server?.uri ?? "";
        user.text = server?.user ?? "";
        domain.text = server?.domain ?? "";
        password.text = "";
        remember.checked = true;
        keep.checked = true;
        root.open();
        (server ? password : address).focusField();
    }
    function go(accept) {
        root.error = "";
        root.busy = true;
        root.job = Ops.connectServer(JSON.stringify({
            address: address.text, user: user.text.trim(), domain: domain.text.trim(), password: password.text,
            remember: remember.checked, save: keep.checked, accept: !!accept,
        }));
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
                root.close();
                root.changed();
                root.opened_(r.path);
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

    parent: Overlay.overlay
    anchors.centerIn: parent
    width: 480
    modal: true
    padding: 20
    background: Rectangle { color: Theme.surface1; radius: 16; border { width: 1; color: Theme.surface2 } }
    Overlay.modal: Rectangle { color: Qt.rgba(0, 0, 0, 0.35) }
    closePolicy: root.busy ? Popup.NoAutoClose : (Popup.CloseOnEscape | Popup.CloseOnPressOutside)

    component Label: Text {
        width: 96
        anchors.verticalCenter: parent.verticalCenter
        color: Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(12) }
    }

    contentItem: Column {
        spacing: 12
        Text { text: "Connect to server"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold } }
        Row {
            spacing: 10
            Label { text: "Address" }
            Field { id: address; width: 330; mono: true; placeholder: "smb://nas/photos"; onAccepted: root.go(false); onTextChanged: root.error = "" }
        }
        Text {
            x: 106; width: 330
            wrapMode: Text.Wrap
            text: "smb://server/share or \\\\server\\share (Windows, Samba) · sftp://user@host (SSH) · ftp:// · davs://host/path (WebDAV) · nfs://"
            color: Theme.faint
            font { family: Theme.font; pointSize: Theme.pt(10.5) }
        }
        Row {
            spacing: 10
            Label { text: "Username" }
            Field { id: user; width: 330; placeholder: root.scheme === "sftp" ? "Yours on the server" : "Leave empty to sign in as a guest"; onAccepted: root.go(false) }
        }
        Row {
            visible: root.scheme === "smb"
            spacing: 10
            Label { text: "Domain" }
            Field { id: domain; width: 330; placeholder: "WORKGROUP (usually right)"; onAccepted: root.go(false) }
        }
        Row {
            spacing: 10
            Label { text: "Password" }
            Field { id: password; width: 330; secret: true; error: root.wrong; placeholder: root.scheme === "sftp" ? "Not needed with an SSH key" : ""; onAccepted: root.go(false); onTextChanged: root.wrong = false }
        }
        Row {
            x: 106
            spacing: 18
            Row {
                spacing: 6
                Toggle { id: remember; checked: true; anchors.verticalCenter: parent.verticalCenter }
                Text { anchors.verticalCenter: parent.verticalCenter; text: "Remember the password"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
            }
            Row {
                spacing: 6
                Toggle { id: keep; checked: true; anchors.verticalCenter: parent.verticalCenter }
                Text { anchors.verticalCenter: parent.verticalCenter; text: "Add to the sidebar"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
            }
        }
        // The server asks something (host key, certificate).
        Rectangle {
            visible: root.question !== ""
            width: parent.width
            height: q.implicitHeight + 24
            radius: 10
            color: Theme.bg
            border { width: 1; color: Theme.rose }
            Text {
                id: q
                x: 12; y: 12
                width: parent.width - 24
                wrapMode: Text.Wrap
                text: root.question + "\n\nOnly trust it if you know this is the right server (on a new server, or after it was set up again, this is normal)."
                color: Theme.text
                font { family: Theme.font; pointSize: Theme.pt(11.5) }
            }
        }
        Text {
            visible: root.error !== ""
            width: parent.width
            wrapMode: Text.Wrap
            text: root.error
            color: Theme.rose
            font { family: Theme.font; pointSize: Theme.pt(11.5) }
        }
        Item {
            width: parent.width; height: 34
            Text {
                visible: root.busy
                anchors { left: parent.left; verticalCenter: parent.verticalCenter }
                text: "Connecting…"
                color: Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(11.5) }
            }
            Row {
                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                spacing: 8
                Button {
                    text: "Cancel"
                    onClicked: {
                        if (root.busy) { Ops.stopCleanup(root.job); root.job = -1; root.busy = false; return }
                        root.close();
                    }
                }
                Button {
                    visible: root.question !== ""
                    text: "Trust and connect"; kind: "danger"
                    enabled: !root.busy
                    onClicked: root.go(true)
                }
                Button {
                    visible: root.question === ""
                    text: "Connect"; kind: "primary"
                    enabled: !root.busy && address.text.trim() !== ""
                    onClicked: root.go(false)
                }
            }
        }
    }
}
