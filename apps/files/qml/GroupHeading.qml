// A group's heading (Group by): its name, how many, and a chevron; click to
// fold the group away or back.
import QtQuick
import WhiteRose.Kit

Item {
    id: root
    property string label
    property int count
    property bool folded: false
    signal toggled()
    Row {
        anchors { left: parent.left; leftMargin: 6; verticalCenter: parent.verticalCenter }
        spacing: 8
        Icon { anchors.verticalCenter: parent.verticalCenter; name: root.folded ? "chevR" : "chevDown"; size: 12; color: Theme.muted }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.label
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(12.5); weight: Font.DemiBold }
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: root.count
            color: Theme.faint
            font { family: Theme.font; pointSize: Theme.pt(11.5) }
        }
    }
    Rectangle {
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom; leftMargin: 6; rightMargin: 6 }
        height: 1
        color: Theme.surface1
    }
    MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: root.toggled() }
}
