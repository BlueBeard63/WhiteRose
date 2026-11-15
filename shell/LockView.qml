// What a locked screen shows: the time, who's logged in, and the password
// field. Typing anywhere goes to the field; Enter checks it. A finger on the
// reader unlocks too, when one is enrolled (Auth.qml).
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

        Avatar {
            anchors.horizontalCenter: parent.horizontalCenter
            width: 64; height: 64
            name: root.name
            initialsSize: 24
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
                onTextChanged: if (text !== "") { Auth.error = ""; Auth.wake() }
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
        // A wrong password, else what the fingerprint reader says.
        Row {
            anchors.horizontalCenter: parent.horizontalCenter
            height: 16
            spacing: 6
            readonly property bool finger: Auth.error === "" && Auth.fingerAvailable && Auth.fingerMessage !== ""
            Icon {
                id: fingerIcon
                anchors.verticalCenter: parent.verticalCenter
                visible: parent.finger
                name: "fingerprint"; size: 15
                color: Auth.fingerError ? Theme.rose : Theme.muted
                // Breathes while the reader listens.
                SequentialAnimation on opacity {
                    running: Auth.fingerListening && !Auth.fingerError
                    loops: Animation.Infinite
                    onRunningChanged: if (!running) fingerIcon.opacity = 1
                    NumberAnimation { to: 0.45; duration: 900; easing.type: Easing.InOutSine }
                    NumberAnimation { to: 1; duration: 900; easing.type: Easing.InOutSine }
                }
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: Auth.error !== "" ? Auth.error : parent.finger ? Auth.fingerMessage : ""
                color: Auth.error !== "" || Auth.fingerError ? Theme.rose : Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(12) }
            }
        }
    }

    Connections {
        target: Auth
        function onErrorChanged() { if (Auth.error !== "") shake.restart() }
    }

    // Any click puts the cursor back in the field (and wakes the reader).
    MouseArea { anchors.fill: parent; z: -1; onClicked: { field.forceActiveFocus(); Auth.wake() } }
    Component.onCompleted: Auth.wake()
}
