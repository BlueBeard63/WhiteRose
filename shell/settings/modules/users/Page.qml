// Users: your account (name, picture, password) and the other people on this
// computer, through AccountsService (whiterosed's users ops). Changes need
// permission: the shell's password prompt appears when they do, and a
// request waits until you answer it.
import QtQuick
import QtQuick.Dialogs
import Quickshell
import Quickshell.Widgets
import qs
import qs.settings
import "../../../QuickSettings.js" as QS
import "Users.js" as U

Column {
    id: page
    spacing: 16

    property var users: []
    property string loadError: ""
    readonly property var me: users.find(u => u.me) ?? null
    readonly property var others: users.filter(u => !u.me)
    readonly property bool admin: me?.admin ?? false

    function refresh() {
        Daemon.request({ op: "users" }, (v, err) => {
            if (!page) return; // (left the page meanwhile)
            page.loadError = err ?? "";
            if (!err) page.users = v ?? [];
        });
    }
    Component.onCompleted: refresh()

    // Runs a users op; `done(error)` afterwards (the list refreshes either way).
    function change(op, args, done) {
        Daemon.request({ op: op, args: args }, (v, err) => {
            page.refresh();
            if (done) done(err ?? "");
        });
    }
    function label(u) { return u.real_name || u.name }

    PageHeader {
        title: "Users"
        subtitle: "People who can sign in to this computer."
        Button { visible: page.admin; text: "Add user"; icon: "plus"; onClicked: addDialog.openNew() }
    }

    SettingRow {
        visible: page.loadError !== ""
        title: "Accounts aren't available"
        subtitle: page.loadError
    }

    // ---- you ------------------------------------------------------------------------
    Card {
        visible: page.me !== null
        Item {
            width: parent.width; height: 104
            Avatar {
                id: myAvatar
                x: 20; anchors.verticalCenter: parent.verticalCenter
                size: 64
                user: page.me
                // Hover: change the picture.
                Rectangle {
                    anchors.fill: parent
                    radius: width / 2
                    visible: avatarMouse.containsMouse
                    color: Qt.rgba(0, 0, 0, 0.55)
                    Icon { anchors.centerIn: parent; name: "camera"; size: 18; color: "white" }
                }
                MouseArea { id: avatarMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: pictureDialog.open() }
            }
            Column {
                anchors { left: myAvatar.right; leftMargin: 18; verticalCenter: parent.verticalCenter }
                spacing: 4
                // The name: click to edit, Enter saves.
                Item {
                    width: Math.max(nameText.implicitWidth, 200); height: 30
                    Text {
                        id: nameText
                        visible: !nameField.visible
                        anchors.verticalCenter: parent.verticalCenter
                        text: page.me ? page.label(page.me) : ""
                        color: Theme.text
                        font { family: Theme.font; pointSize: Theme.pt(18); weight: Font.DemiBold }
                        MouseArea { anchors.fill: parent; cursorShape: Qt.IBeamCursor; onClicked: { nameField.text = page.me.real_name; nameField.visible = true; nameField.focusField() } }
                    }
                    Field {
                        id: nameField
                        visible: false
                        width: 280
                        placeholder: "Your name"
                        onAccepted: {
                            visible = false;
                            if (text.trim() !== page.me.real_name) page.change("user_name", { uid: page.me.uid, name: text.trim() });
                        }
                    }
                }
                Text {
                    text: (page.me?.name ?? "") + " · " + (page.admin ? "Administrator" : "Standard user")
                    color: Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(12) }
                }
            }
            Row {
                anchors { right: parent.right; rightMargin: 18; verticalCenter: parent.verticalCenter }
                spacing: 8
                Button { text: "Fingerprints"; icon: "fingerprint"; onClicked: SettingsState.open("users/fingerprints") }
                Button { text: "Change password"; icon: "lock"; onClicked: passwordDialog.openFor(page.me) }
            }
        }
    }

    // ---- other people ----------------------------------------------------------------------
    Section { visible: page.others.length > 0; text: "Other people" }
    Card {
        visible: page.others.length > 0
        Repeater {
            model: page.others
            delegate: Item {
                id: row
                required property var modelData
                required property int index
                width: parent.width; height: 60
                Rectangle { visible: row.index > 0; width: parent.width; height: 1; color: Theme.surface2 }
                Avatar { id: av; x: 16; anchors.verticalCenter: parent.verticalCenter; size: 36; user: row.modelData }
                Column {
                    anchors { left: av.right; leftMargin: 12; verticalCenter: parent.verticalCenter }
                    spacing: 2
                    Row {
                        spacing: 8
                        Text { text: page.label(row.modelData); color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.Medium } }
                        Badge { visible: row.modelData.admin; text: "admin" }
                        Badge { visible: row.modelData.locked; text: "locked" }
                    }
                    Text {
                        text: row.modelData.name + " · " + U.lastSeen(row.modelData.last_login, Date.now())
                        color: Theme.muted
                        font { family: Theme.font; pointSize: Theme.pt(11.5) }
                    }
                }
                Choice {
                    visible: page.admin
                    anchors { right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
                    value: ""
                    options: [
                        { label: "Manage…", value: "" },
                        { label: row.modelData.admin ? "Make standard user" : "Make administrator", value: "admin" },
                        { label: "Set password…", value: "password" },
                        { label: "Remove…", value: "remove" },
                    ]
                    onPicked: v => {
                        if (v === "admin") page.change("user_admin", { uid: row.modelData.uid, admin: !row.modelData.admin });
                        else if (v === "password") passwordDialog.openFor(row.modelData);
                        else if (v === "remove") removeDialog.openFor(row.modelData);
                    }
                }
            }
        }
    }
    Text {
        visible: !page.admin && page.me !== null
        width: parent.width
        wrapMode: Text.Wrap
        text: "Only administrators can add people or change other accounts."
        color: Theme.faint
        font { family: Theme.font; pointSize: Theme.pt(11.5) }
    }

    // ---- avatar --------------------------------------------------------------------------------
    component Avatar: Item {
        id: avatar
        property var user: null
        property real size: 36
        width: size; height: size
        ClippingRectangle {
            anchors.fill: parent
            radius: width / 2
            color: Theme.surface2
            Text {
                anchors.centerIn: parent
                visible: pic.status !== Image.Ready
                text: avatar.user ? QS.initials(avatar.user.real_name || avatar.user.name).toUpperCase() : ""
                color: Theme.text
                font { family: Theme.font; pointSize: Math.max(1, Theme.pt(avatar.size * 0.36)); weight: Font.Medium }
            }
            Image {
                id: pic
                anchors.fill: parent
                source: avatar.user?.icon ? "file://" + avatar.user.icon : ""
                sourceSize: Qt.size(avatar.size * 2, avatar.size * 2)
                fillMode: Image.PreserveAspectCrop
                asynchronous: true
                cache: false
                visible: status === Image.Ready
            }
        }
    }

    // ---- dialogs --------------------------------------------------------------------------------
    FileDialog {
        id: pictureDialog
        title: "Choose a picture"
        currentFolder: "file://" + (Quickshell.env("HOME") ?? "") + "/Pictures"
        nameFilters: ["Images (*.png *.jpg *.jpeg *.webp *.bmp)"]
        onAccepted: page.change("user_icon", { uid: page.me.uid, path: decodeURIComponent(String(selectedFile).replace(/^file:\/\//, "")) })
    }

    Dialog {
        id: passwordDialog
        property var user: null
        property bool waiting: false
        property string error: ""
        function openFor(u) { user = u; pw1.text = ""; pw2.text = ""; error = ""; waiting = false; open(); pw1.focusField() }
        readonly property string problem: U.passwordProblem(pw1.text, pw2.text)
        title: user?.me ? "Change your password" : "Set a password for " + (user ? page.label(user) : "")
        subtitle: user?.me ? "You'll be asked for your current one" : "They can change it after signing in"
        icon: "lock"
        width: 440
        Field { id: pw1; width: parent.width; secret: true; placeholder: "New password"; onAccepted: pw2.focusField() }
        Field { id: pw2; width: parent.width; secret: true; placeholder: "Again"; error: pw2.text !== "" && pw2.text !== pw1.text; onAccepted: saveBtn.clicked() }
        Text {
            visible: text !== ""
            width: parent.width
            wrapMode: Text.Wrap
            text: passwordDialog.error || (pw1.text !== "" ? passwordDialog.problem : "")
            color: passwordDialog.error ? Theme.rose : Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(11.5) }
        }
        footer: [
            Item { width: parent.width - pwCancel.width - saveBtn.width - 16; height: 1 },
            Button { id: pwCancel; text: "Cancel"; onClicked: passwordDialog.close() },
            Button {
                id: saveBtn
                text: passwordDialog.waiting ? "Waiting for permission…" : "Change password"
                kind: "primary"
                enabled: passwordDialog.problem === "" && !passwordDialog.waiting
                onClicked: {
                    if (!enabled) return;
                    passwordDialog.waiting = true;
                    passwordDialog.error = "";
                    page.change("user_password", { uid: passwordDialog.user.uid, password: pw1.text }, err => {
                        passwordDialog.waiting = false;
                        if (err) passwordDialog.error = err; else passwordDialog.close();
                    });
                }
            }
        ]
    }

    Dialog {
        id: addDialog
        property bool waiting: false
        property string error: ""
        property bool nameEdited: false
        function openNew() {
            fullName.text = ""; userName.text = ""; newPw1.text = ""; newPw2.text = "";
            nameEdited = false; error = ""; waiting = false; adminChoice.value = false;
            open(); fullName.focusField();
        }
        readonly property string nameProblem: U.nameProblem(userName.text, page.users.map(u => u.name))
        readonly property string pwProblem: U.passwordProblem(newPw1.text, newPw2.text)
        title: "Add a user"
        subtitle: "They'll sign in with the username and password"
        icon: "user"
        width: 460
        Field {
            id: fullName; width: parent.width; placeholder: "Full name"
            onTextChanged: if (!addDialog.nameEdited) userName.text = U.suggestName(text)
            onAccepted: userName.focusField()
        }
        Field {
            id: userName; width: parent.width; mono: true; placeholder: "username"
            error: text !== "" && addDialog.nameProblem !== ""
            // Typed by hand (not the suggestion): stop following the full name.
            onTextChanged: if (text !== U.suggestName(fullName.text)) addDialog.nameEdited = true
            onAccepted: newPw1.focusField()
        }
        Field { id: newPw1; width: parent.width; secret: true; placeholder: "Password"; onAccepted: newPw2.focusField() }
        Field { id: newPw2; width: parent.width; secret: true; placeholder: "Password again"; error: newPw2.text !== "" && newPw2.text !== newPw1.text; onAccepted: createBtn.clicked() }
        Row {
            spacing: 10
            Text { anchors.verticalCenter: parent.verticalCenter; text: "Account"; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(12) } }
            Segmented {
                id: adminChoice
                value: false
                options: [{ label: "Standard", value: false }, { label: "Administrator", value: true }]
                onPicked: v => value = v
            }
        }
        Text {
            visible: text !== ""
            width: parent.width
            wrapMode: Text.Wrap
            text: addDialog.error || (userName.text !== "" && addDialog.nameProblem) || (newPw1.text !== "" ? addDialog.pwProblem : "")
            color: addDialog.error ? Theme.rose : Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(11.5) }
        }
        footer: [
            Item { width: parent.width - addCancel.width - createBtn.width - 16; height: 1 },
            Button { id: addCancel; text: "Cancel"; onClicked: addDialog.close() },
            Button {
                id: createBtn
                text: addDialog.waiting ? "Waiting for permission…" : "Add user"
                kind: "primary"
                enabled: addDialog.nameProblem === "" && addDialog.pwProblem === "" && !addDialog.waiting
                onClicked: {
                    if (!enabled) return;
                    addDialog.waiting = true;
                    addDialog.error = "";
                    page.change("user_create", { name: userName.text, real_name: fullName.text.trim(), admin: adminChoice.value, password: newPw1.text }, err => {
                        addDialog.waiting = false;
                        if (err) addDialog.error = err; else addDialog.close();
                    });
                }
            }
        ]
    }

    Dialog {
        id: removeDialog
        property var user: null
        property bool waiting: false
        property string error: ""
        function openFor(u) { user = u; keepFiles.checked = true; error = ""; waiting = false; open() }
        title: "Remove " + (user ? page.label(user) : "") + "?"
        subtitle: "They won't be able to sign in any more"
        icon: "trash"
        width: 440
        SettingRow {
            divider: false
            title: "Keep their files"
            subtitle: removeDialog.user ? "/home/" + removeDialog.user.name + " stays on this computer" : ""
            Toggle { id: keepFiles; checked: true; onToggled: v => checked = v }
        }
        Text {
            visible: removeDialog.error !== ""
            width: parent.width
            wrapMode: Text.Wrap
            text: removeDialog.error
            color: Theme.rose
            font { family: Theme.font; pointSize: Theme.pt(11.5) }
        }
        footer: [
            Item { width: parent.width - rmCancel.width - rmBtn.width - 16; height: 1 },
            Button { id: rmCancel; text: "Cancel"; onClicked: removeDialog.close() },
            Button {
                id: rmBtn
                text: removeDialog.waiting ? "Waiting for permission…" : "Remove"
                kind: "danger"
                enabled: !removeDialog.waiting
                onClicked: {
                    removeDialog.waiting = true;
                    page.change("user_delete", { uid: removeDialog.user.uid, keep_files: keepFiles.checked }, err => {
                        removeDialog.waiting = false;
                        if (err) removeDialog.error = err; else removeDialog.close();
                    });
                }
            }
        ]
    }
}
