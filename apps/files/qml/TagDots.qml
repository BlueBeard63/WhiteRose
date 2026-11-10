// A file's tags as small coloured dots (their names on hover).
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Files

Row {
    id: root
    property var tags: []
    property real size: 8
    spacing: 3
    visible: root.tags.length > 0
    Repeater {
        model: root.tags.slice(0, 5)
        delegate: Rectangle {
            required property string modelData
            width: root.size; height: root.size; radius: root.size / 2
            color: Session.tagColour(modelData)
        }
    }
    HoverHandler { id: hover }
    ToolTip.visible: hover.hovered && root.tags.length > 0
    ToolTip.delay: 400
    ToolTip.text: root.tags.join(", ")
}
