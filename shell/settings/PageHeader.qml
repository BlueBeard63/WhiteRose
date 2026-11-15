// A page's title and description, with its buttons (Add user, Install…)
// on the right; where the window is too narrow for both (a tiled Settings
// window), the buttons go under the description and the description wraps.
import QtQuick
import qs

Item {
    id: root
    property string title
    property string subtitle
    default property alias actions: actionRow.data
    width: parent?.width ?? 0
    readonly property bool stacked: actionRow.width > 0 && titleText.implicitWidth + 12 + actionRow.width > width
    implicitHeight: (stacked ? actionRow.y + actionRow.height : subtitleText.y + subtitleText.height)

    Text {
        id: titleText
        width: root.stacked ? root.width : root.width - (actionRow.width > 0 ? actionRow.width + 12 : 0)
        text: root.title
        elide: Text.ElideRight
        color: Theme.text
        font { family: Theme.font; pointSize: Theme.pt(22); weight: Font.DemiBold }
    }
    Text {
        id: subtitleText
        y: titleText.height + 4
        width: root.width
        height: visible ? implicitHeight : 0
        visible: text !== ""
        text: root.subtitle
        wrapMode: Text.Wrap
        color: Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(12) }
    }
    Row {
        id: actionRow
        x: root.stacked ? 0 : root.width - width
        y: root.stacked ? subtitleText.y + subtitleText.height + 10 : Math.round((titleText.height - height) / 2)
        spacing: 8
    }
}
