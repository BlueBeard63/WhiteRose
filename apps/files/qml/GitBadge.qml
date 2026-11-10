// A file's git state as a small mark: M modified, + new (staged), U
// untracked, ! conflict (nothing for ignored: those are dimmed instead).
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit

Rectangle {
    id: root
    property string state: ""
    property real size: 14
    readonly property var look: ({
        modified: { text: "M", colour: "#E3B341", tip: "Modified" },
        added: { text: "+", colour: "#57AB5A", tip: "New (staged)" },
        untracked: { text: "U", colour: "#57AB5A", tip: "Untracked" },
        conflict: { text: "!", colour: "#E5534B", tip: "Conflict" },
    })[root.state] ?? null
    visible: root.look !== null
    width: root.size; height: root.size; radius: root.size * 0.3
    color: Theme.bg
    border { width: 1; color: root.look?.colour ?? "transparent" }
    Text {
        anchors.centerIn: parent
        text: root.look?.text ?? ""
        color: root.look?.colour ?? "transparent"
        font { family: Theme.mono; pixelSize: root.size * 0.68; weight: Font.Bold }
    }
    HoverHandler { id: hover }
    ToolTip.visible: hover.hovered
    ToolTip.delay: 400
    ToolTip.text: root.look?.tip ?? ""
}
