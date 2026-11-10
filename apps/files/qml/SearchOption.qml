// A small on/off chip in the search row ("Aa" match case, ".*" regular expression).
import QtQuick
import QtQuick.Controls
import WhiteRose.Kit

Rectangle {
    id: root
    property string text
    property string tip
    property bool on: false
    signal toggled(bool on)

    width: 34; height: 30; radius: 9
    anchors.verticalCenter: parent?.verticalCenter
    color: root.on ? Theme.pearl : mouse.containsMouse ? Theme.surface2 : Theme.surface1
    Text {
        anchors.centerIn: parent
        text: root.text
        color: root.on ? Theme.bg : Theme.muted
        font { family: Theme.mono; pointSize: Theme.pt(11.5); weight: Font.Medium }
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.toggled(!root.on)
    }
    ToolTip.visible: mouse.containsMouse && root.tip !== ""
    ToolTip.text: root.tip
    ToolTip.delay: 500
}
