// Island view: the keyring is locked (Figma: "Island v2 - Keyring (after a
// fingerprint login)"). A fingerprint login has no password to open it
// with, so calendars and saved passwords wait. Asked once a login
// (ShellState.offerKeyring); the password goes to whiterosed once
// (keyring_unlock) and is never kept. Not now, Esc or clicking away
// closes it for this login.
import QtQuick
import qs
import qs.settings

Item {
    id: root
    implicitWidth: 560
    implicitHeight: body.implicitHeight + 36

    property string error: ""
    property bool busy: false

    function open() {
        if (busy || field.text === "") return;
        busy = true;
        error = "";
        const password = field.text;
        field.text = "";
        Daemon.request({ op: "keyring_unlock", args: { password: password } }, (v, err) => {
            if (!root) return;
            root.busy = false;
            if (err) {
                root.error = "That's not your keyring's password. If you changed your login password, type the old one.";
                field.forceActiveFocus();
                return;
            }
            ShellState.close();
        });
    }

    Component.onCompleted: field.forceActiveFocus()

    Column {
        id: body
        x: 24; y: 18
        width: parent.width - 48
        spacing: 14

        Column {
            width: parent.width
            spacing: 4
            Text {
                text: "Your keyring is locked"
                color: Theme.text
                font { family: Theme.font; pointSize: Theme.pt(20); weight: Font.DemiBold }
                Accessible.role: Accessible.Heading
                Accessible.name: text
            }
            Text {
                width: parent.width
                wrapMode: Text.Wrap
                text: "A fingerprint can't open it. Type your password once and calendars and saved passwords work again until you sign out."
                color: Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(12) }
            }
        }

        Row {
            width: parent.width
            spacing: 8
            Rectangle {
                width: parent.width - notNow.width - openButton.width - 16
                height: 34
                radius: 8
                color: Theme.surface1
                border { width: 1; color: field.activeFocus ? Theme.muted : Theme.surface2 }
                TextInput {
                    id: field
                    anchors { fill: parent; leftMargin: 12; rightMargin: 12 }
                    verticalAlignment: TextInput.AlignVCenter
                    echoMode: TextInput.Password
                    passwordCharacter: "•"
                    color: Theme.text
                    selectionColor: Theme.rose
                    enabled: !root.busy
                    font { family: Theme.font; pointSize: Theme.pt(13) }
                    focus: true
                    onAccepted: root.open()
                    Keys.onEscapePressed: ShellState.close()
                    // (Not Accessible.passwordEdit: Qt then drops the name. The
                    // echo mode already keeps what's typed out of the tree.)
                    Accessible.role: Accessible.EditableText
                    Accessible.name: "Password"
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: field.text === ""
                        text: "Password"
                        color: Theme.faint
                        font: field.font
                    }
                }
            }
            Button {
                id: notNow
                text: "Not now"
                onClicked: ShellState.close()
            }
            Button {
                id: openButton
                text: "Open keyring"
                kind: "primary"
                enabled: !root.busy && field.text !== ""
                onClicked: root.open()
            }
        }

        Text {
            visible: root.error !== ""
            width: parent.width
            wrapMode: Text.Wrap
            text: root.error
            color: Theme.rose
            font { family: Theme.font; pointSize: Theme.pt(12) }
            Accessible.role: Accessible.AlertMessage
            Accessible.name: text
        }
    }
}
