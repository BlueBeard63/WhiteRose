// What a locked screen shows: the time, who's logged in, and the password
// field. Typing anywhere goes to the field; Enter checks it.
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Widgets
import "QuickSettings.js" as QS

Item {
    id: root
    anchors.fill: parent

    readonly property string user: Quickshell.env("USER") ?? ""
    FileView { id: passwd; path: "/etc/passwd" }
    readonly property string name: QS.displayName(passwd.text(), user)
    readonly property var avatars: ["/var/lib/AccountsService/icons/" + user, Quickshell.env("HOME") + "/.face"]

    SystemClock { id: clock; precision: SystemClock.Minutes }

    Column {
        anchors.centerIn: parent
        anchors.verticalCenterOffset: -40
        spacing: 6
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: Qt.formatTime(clock.date, "HH:mm")
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(88); weight: Font.DemiBold }
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: Qt.formatDate(clock.date, "dddd d MMMM")
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(16) }
        }
    }

    Column {
        anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: parent.height * 0.18 }
        spacing: 14

        ClippingRectangle {
            anchors.horizontalCenter: parent.horizontalCenter
            width: 64; height: 64; radius: 32
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
                font { family: Theme.font; pointSize: Theme.pt(24); weight: Font.DemiBold }
            }
            Image {
                id: picture
                property int tried: 0
                anchors.fill: parent
                source: "file://" + root.avatars[tried]
                sourceSize: Qt.size(128, 128)
                fillMode: Image.PreserveAspectCrop
                visible: status === Image.Ready
                onStatusChanged: if (status === Image.Error && tried + 1 < root.avatars.length) tried++
            }
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            text: root.name
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(15); weight: Font.Medium }
        }

        // Password pill.
        Rectangle {
            id: pill
            anchors.horizontalCenter: parent.horizontalCenter
            width: 280; height: 44; radius: 22
            color: Theme.surface1
            border { width: 1; color: Auth.error !== "" ? Theme.rose : field.activeFocus ? Theme.surface2 : "transparent" }

            TextInput {
                id: field
                anchors { fill: parent; leftMargin: 20; rightMargin: 48 }
                verticalAlignment: TextInput.AlignVCenter
                echoMode: TextInput.Password
                passwordCharacter: "•"
                color: Theme.text
                font { family: Theme.font; pointSize: Theme.pt(15); letterSpacing: 2 }
                focus: true
                enabled: !Auth.busy
                onAccepted: { Auth.tryUnlock(text); text = "" }
                onTextChanged: if (text !== "") Auth.error = ""
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: field.text === ""
                    text: Auth.busy ? "Checking…" : "Password"
                    color: Theme.faint
                    font { family: Theme.font; pointSize: Theme.pt(14) }
                }
            }
            IconButton {
                anchors { right: parent.right; rightMargin: 6; verticalCenter: parent.verticalCenter }
                width: 32; height: 32; icon: "chevR"; iconSize: 15
                fill: Theme.surface2; hoverFill: Theme.rose
                iconColor: hovered ? Theme.bg : Theme.text
                onClicked: { Auth.tryUnlock(field.text); field.text = "" }
            }
            // Shake on a wrong password.
            SequentialAnimation on anchors.horizontalCenterOffset {
                id: shake
                running: false
                NumberAnimation { to: -10; duration: 50 }
                NumberAnimation { to: 10; duration: 70 }
                NumberAnimation { to: -6; duration: 60 }
                NumberAnimation { to: 0; duration: 50 }
            }
        }
        Text {
            anchors.horizontalCenter: parent.horizontalCenter
            height: 16
            text: Auth.error
            color: Theme.rose
            font { family: Theme.font; pointSize: Theme.pt(12) }
        }
    }

    Connections {
        target: Auth
        function onErrorChanged() { if (Auth.error !== "") shake.restart() }
    }

    // Any click puts the cursor back in the field.
    MouseArea { anchors.fill: parent; z: -1; onClicked: field.forceActiveFocus() }
}
