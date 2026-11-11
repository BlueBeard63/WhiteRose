// One setting row: title, optional explanation, and its control on the right.
// Rows after the first in a card draw a hairline above themselves.
import QtQuick
import qs

Item {
    id: root
    property string title
    property string subtitle
    property bool divider: true
    property bool dimmed: false
    default property alias control: controls.data
    width: parent?.width ?? 0
    implicitHeight: Math.max(subtitle ? 56 : 46, text.implicitHeight + 20, controls.implicitHeight + 16)
    opacity: dimmed ? 0.45 : 1

    Rectangle {
        visible: root.divider && root.y > 0
        width: parent.width; height: 1
        color: Theme.surface2
    }
    Column {
        id: text
        anchors { left: parent.left; leftMargin: 14; right: controls.left; rightMargin: 14; verticalCenter: parent.verticalCenter }
        spacing: 2
        Text {
            width: parent.width
            text: root.title
            elide: Text.ElideRight
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.Medium }
        }
        Text {
            width: parent.width
            visible: root.subtitle !== ""
            text: root.subtitle
            wrapMode: Text.Wrap
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(11.5) }
        }
    }
    Row {
        id: controls
        // What its toggle, dropdown or slider is called (screen readers, tests).
        readonly property string rowTitle: root.title
        anchors { right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
        spacing: 8
    }
}
