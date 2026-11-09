// An app's icon from its desktop entry and the icon theme, or its initial
// when the theme has none (never a broken image).
import QtQuick
import Quickshell

Item {
    id: root
    property var entry: null
    property string name: ""
    property real size: 26
    width: size
    height: size

    Image {
        id: img
        anchors.fill: parent
        source: root.entry ? Quickshell.iconPath(root.entry.icon, true) : ""
        sourceSize: Qt.size(root.size * 2, root.size * 2)
        asynchronous: true
        visible: status === Image.Ready
    }
    Text {
        anchors.centerIn: parent
        visible: img.status !== Image.Ready
        text: (root.name || "?").charAt(0).toUpperCase()
        color: Theme.text
        font { family: Theme.font; pointSize: Theme.pt(root.size * 0.55); weight: Font.DemiBold }
    }
}
