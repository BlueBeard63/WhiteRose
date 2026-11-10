// Add cloud account: Google Drive, OneDrive, Dropbox, iCloud Drive, S3 or
// pCloud, through rclone (cloudremotes.rs). Accounts that sign in with a
// browser open it; anything rclone needs to ask (an iCloud two-factor
// code, a choice) is asked here, one question at a time. Without rclone:
// a way to install it (Settings › Apps).
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files

Popup {
    id: root
    signal added(string name)

    property var info: ({ rclone: false, providers: [] })
    property string kind: "drive"
    property int job: -1
    property bool busy: false
    property string error: ""
    // A sign-in link rclone gave (if the browser didn't open).
    property string link: ""
    // rclone's question: {state, name, help, default, choices, password, required}.
    property var ask: null
    // The choice picked for a question with choices (starts at its default).
    property string pick: ""
    readonly property bool browserSignIn: ["drive", "onedrive", "dropbox", "pcloud"].includes(root.kind)

    function show() {
        root.info = JSON.parse(Ops.cloudProviders());
        root.kind = "drive";
        name.text = "Google Drive";
        root.reset();
        root.open();
    }
    function reset() { root.error = ""; root.link = ""; root.ask = null; root.busy = false; answer.text = "" }
    function params() {
        if (root.kind === "iclouddrive") return { apple_id: appleId.text.trim(), password: applePassword.text };
        if (root.kind === "s3") return { provider: s3Provider.value, access_key_id: s3Key.text.trim(), secret_access_key: s3Secret.text, endpoint: s3Endpoint.text.trim(), region: s3Region.text.trim(), env_auth: "false" };
        return {};
    }
    function start() {
        root.reset();
        root.busy = true;
        root.job = Ops.cloudSetup(JSON.stringify({ name: name.text.trim(), kind: root.kind, params: root.params() }));
    }
    function next(value) {
        const a = root.ask;
        root.ask = null;
        root.error = "";
        root.busy = true;
        root.job = Ops.cloudSetup(JSON.stringify({ name: name.text.trim(), state: a.state, answer: value }));
    }
    Connections {
        target: Ops
        function onCloudSetupStep(id, result) {
            if (id !== root.job) return;
            const r = JSON.parse(result);
            if (r.note) { root.link = r.note; return }
            root.busy = false;
            if (r.step === "done") { root.close(); root.added(name.text.trim()) }
            else if (r.step === "ask") { root.ask = r; root.pick = r.default || (r.choices[0]?.[0] ?? ""); answer.text = r.choices.length ? "" : r.default; if (!r.choices.length) answer.focusField() }
            else root.error = r.error;
        }
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    width: 540
    modal: true
    padding: 20
    background: Rectangle { color: Theme.surface1; radius: 16; border { width: 1; color: Theme.surface2 } }
    Overlay.modal: Rectangle { color: Qt.rgba(0, 0, 0, 0.35) }
    closePolicy: root.busy ? Popup.NoAutoClose : (Popup.CloseOnEscape | Popup.CloseOnPressOutside)

    component Label: Text {
        width: 110
        anchors.verticalCenter: parent.verticalCenter
        color: Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(12) }
    }
    component Note: Text {
        width: parent.width
        wrapMode: Text.Wrap
        color: Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(11.5) }
    }

    contentItem: Column {
        spacing: 12
        Text { text: "Add a cloud account"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold } }

        // ---- no rclone yet ----
        Column {
            visible: !root.info.rclone
            width: parent.width
            spacing: 12
            Note { text: "Cloud drives in Files (Google Drive, OneDrive, Dropbox, iCloud Drive, S3) use rclone, which isn't installed. Settings › Apps can install it." }
            Row {
                spacing: 8
                Button { text: "Install rclone…"; kind: "primary"; onClicked: { const e = Fs.openSettings("apps"); if (e) root.error = e } }
                Button { text: "Check again"; onClicked: root.info = JSON.parse(Ops.cloudProviders()) }
            }
        }

        // ---- which account ----
        Column {
            visible: root.info.rclone && root.ask === null
            width: parent.width
            spacing: 12
            Row {
                spacing: 10
                Label { text: "Account" }
                Choice {
                    value: root.kind
                    options: root.info.providers.map(p => ({ label: p.name, value: p.kind }))
                    onPicked: v => {
                        // The name follows the kind until it's typed.
                        const before = root.info.providers.find(p => p.kind === root.kind);
                        root.kind = v;
                        const now = root.info.providers.find(p => p.kind === v);
                        if (name.text === "" || (before && name.text === before.name.split(" (")[0])) name.text = now.name.split(" (")[0];
                    }
                }
            }
            Row {
                spacing: 10
                Label { text: "Name" }
                Field { id: name; width: 300; placeholder: "Shown in the sidebar" }
            }
            Row {
                visible: root.kind === "iclouddrive"
                spacing: 10
                Label { text: "Apple ID" }
                Field { id: appleId; width: 300; placeholder: "name@icloud.com" }
            }
            Row {
                visible: root.kind === "iclouddrive"
                spacing: 10
                Label { text: "Password" }
                Field { id: applePassword; width: 300; secret: true }
            }
            Note {
                visible: root.kind === "iclouddrive"
                text: "Advanced Data Protection must be off for iCloud Drive to be reachable. A code will come to your Apple devices next."
            }
            Row {
                visible: root.kind === "s3"
                spacing: 10
                Label { text: "Service" }
                Choice {
                    id: s3Provider
                    value: "AWS"
                    options: [{ label: "Amazon S3", value: "AWS" }, { label: "Cloudflare R2", value: "Cloudflare" }, { label: "Wasabi", value: "Wasabi" }, { label: "MinIO", value: "Minio" }, { label: "Another (S3-compatible)", value: "Other" }]
                    onPicked: v => value = v
                }
            }
            Row {
                visible: root.kind === "s3"
                spacing: 10
                Label { text: "Access key" }
                Field { id: s3Key; width: 300; mono: true }
            }
            Row {
                visible: root.kind === "s3"
                spacing: 10
                Label { text: "Secret key" }
                Field { id: s3Secret; width: 300; mono: true; secret: true }
            }
            Row {
                visible: root.kind === "s3"
                spacing: 10
                Label { text: "Endpoint" }
                Field { id: s3Endpoint; width: 300; mono: true; placeholder: s3Provider.value === "AWS" ? "(not needed for Amazon)" : "https://…" }
            }
            Row {
                visible: root.kind === "s3"
                spacing: 10
                Label { text: "Region" }
                Field { id: s3Region; width: 300; mono: true; placeholder: "e.g. eu-west-2 (optional)" }
            }
            Note {
                visible: root.browserSignIn
                text: "Your web browser opens to sign in. Come back here when it says you're done."
            }
        }

        // ---- rclone asks something ----
        Column {
            visible: root.ask !== null
            width: parent.width
            spacing: 10
            Flickable {
                width: parent.width
                height: Math.min(helpText.implicitHeight, 200)
                contentHeight: helpText.implicitHeight
                clip: true
                Text {
                    id: helpText
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: root.ask?.help ?? ""
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(12) }
                }
            }
            Row {
                spacing: 8
                readonly property bool choosing: (root.ask?.choices ?? []).length > 0
                Choice {
                    visible: parent.choosing
                    value: root.pick
                    options: (root.ask?.choices ?? []).map(c => ({ label: c[1] ? c[1].split("\n")[0] : c[0], value: c[0] }))
                    onPicked: v => root.pick = v
                }
                Field { id: answer; visible: !parent.choosing; width: 360; secret: root.ask?.password ?? false; onAccepted: root.next(answer.text) }
                Button {
                    text: "Continue"; kind: "primary"
                    enabled: parent.choosing || !(root.ask?.required ?? false) || answer.text !== ""
                    onClicked: root.next(parent.choosing ? root.pick : answer.text)
                }
            }
        }

        // ---- signing in ----
        Note {
            visible: root.busy
            text: root.browserSignIn && root.ask === null ? "Waiting for you to sign in in your browser…" : "Setting it up…"
        }
        Text {
            visible: root.busy && root.link !== ""
            width: parent.width
            wrapMode: Text.WrapAnywhere
            textFormat: Text.StyledText
            text: "If your browser didn't open: <a href=\"" + root.link + "\">" + root.link + "</a>"
            linkColor: Theme.rose
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(11) }
            onLinkActivated: url => Qt.openUrlExternally(url)
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
            Row {
                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                spacing: 8
                Button {
                    text: "Cancel"
                    onClicked: {
                        if (root.busy) { Ops.stopCleanup(root.job); root.busy = false; return }
                        root.close();
                    }
                }
                Button {
                    visible: root.info.rclone && root.ask === null
                    text: root.browserSignIn ? "Sign in" : "Add"
                    kind: "primary"
                    enabled: !root.busy && name.text.trim() !== ""
                    onClicked: root.start()
                }
            }
        }
    }
}
