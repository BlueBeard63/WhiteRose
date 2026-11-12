// One setting row: title, optional explanation, and its control on the right.
// Where the row is too narrow for both side by side (a tiled Settings window,
// Figma: "Settings - narrow · third"), the controls go under the title.
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
    // Side by side needs room for the controls and a readable title.
    readonly property bool stacked: controls.implicitWidth > 0 && width - 42 - controls.implicitWidth < 160
    implicitHeight: stacked
        ? text.implicitHeight + controls.implicitHeight + 34
        : Math.max(subtitle ? 56 : 46, text.implicitHeight + 20, controls.implicitHeight + 16)
    opacity: dimmed ? 0.45 : 1

    Rectangle {
        visible: root.divider && root.y > 0
        width: parent.width; height: 1
        color: Theme.surface2
    }
    Column {
        id: text
        x: 14
        y: root.stacked ? 12 : Math.round((root.height - height) / 2)
        width: root.stacked ? root.width - 28 : controls.x - 28
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
        x: root.stacked ? 14 : root.width - 14 - width
        y: root.stacked ? text.y + text.height + 10 : Math.round((root.height - height) / 2)
        spacing: 8
    }
}
