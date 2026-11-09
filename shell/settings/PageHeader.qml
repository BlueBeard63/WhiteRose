// A page's title and one-line description.
import QtQuick
import qs

Column {
    property string title
    property string subtitle
    default property alias actions: actionRow.data
    width: parent?.width ?? 0
    spacing: 4
    Item {
        width: parent.width
        height: titleText.implicitHeight
        Text {
            id: titleText
            text: parent.parent.title
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(22); weight: Font.DemiBold }
        }
        Row { id: actionRow; anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter; spacing: 8 }
    }
    Text {
        visible: text !== ""
        text: parent.subtitle
        color: Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(12) }
    }
}
