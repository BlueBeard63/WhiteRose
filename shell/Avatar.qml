// The user's picture, round, or their initials on the rose gradient. The
// picture is AccountsService's (what Settings › Users and GNOME set), else
// ~/.face. Only one that exists is loaded, so a missing picture isn't an
// error in the log. Looked for again each time it shows.
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Widgets
import "QuickSettings.js" as QS

ClippingRectangle {
    id: root
    // Login name ($USER) and the name to take initials from.
    property string user: Quickshell.env("USER") ?? ""
    property string name
    property real initialsSize: 14
    radius: width / 2
    Accessible.role: Accessible.Graphic
    Accessible.name: "Profile picture"
    Accessible.description: picture.status === Image.Ready ? "Picture" : "Initials"

    property string path: ""
    // Looked for when it shows (a picture set since then shows too).
    onVisibleChanged: if (visible) look.running = true
    Component.onCompleted: look.running = true
    Process {
        id: look
        command: ["sh", "-c", "for f; do [ -r \"$f\" ] && [ -s \"$f\" ] && { printf %s \"$f\"; exit; }; done; true", "sh",
            "/var/lib/AccountsService/icons/" + root.user, Quickshell.env("HOME") + "/.face"]
        stdout: StdioCollector { onStreamFinished: root.path = text }
    }

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
        font { family: Theme.font; pointSize: Theme.pt(root.initialsSize); weight: Font.DemiBold }
    }
    Image {
        id: picture
        anchors.fill: parent
        source: root.path ? "file://" + root.path : ""
        sourceSize: Qt.size(root.width * 2, root.height * 2)
        fillMode: Image.PreserveAspectCrop
        visible: status === Image.Ready
    }
}
