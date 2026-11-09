// An app's icon from its desktop entry and the icon theme, or its initial
// when the theme has none (never a broken image).
import QtQuick
import Quickshell

Item {
    id: root
    property var entry: null
    property string name: ""
    // An icon-theme name to use without a desktop entry (an app not
    // installed yet).
    property string iconName: ""
    property real size: 26
    width: size
    height: size

    Image {
        id: img
        anchors.fill: parent
        source: root.entry ? Quickshell.iconPath(root.entry.icon, true) : root.iconName ? Quickshell.iconPath(root.iconName, true) : ""
        sourceSize: Qt.size(root.size * 2, root.size * 2)
        // Theme icons are small and come from QuickShell's icon cache: loading
        // them in place means a rebuilt row (the launcher, every keystroke)
        // shows its icon at once instead of flashing the letter first.
        asynchronous: false
        visible: status === Image.Ready
    }
    Text {
        anchors.centerIn: parent
        visible: img.status === Image.Error || img.source == ""
        text: (root.name || "?").charAt(0).toUpperCase()
        color: Theme.text
        font { family: Theme.font; pointSize: Theme.pt(root.size * 0.55); weight: Font.DemiBold }
    }
}
