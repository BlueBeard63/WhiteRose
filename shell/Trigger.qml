// A clickable area in the bar (Figma: Trigger/*). Highlights on hover and
// while its panel is open.
import QtQuick

Rectangle {
    id: root
    default property alias content: row.data
    property bool active: false
    property int spacing: 6
    // What it is (screen readers, UI tests), and what it shows now.
    property string label: ""
    property string shows: ""
    signal clicked()
    Accessible.role: Accessible.Button
    Accessible.name: root.label
    Accessible.description: root.shows
    Accessible.onPressAction: root.clicked()

    implicitWidth: row.implicitWidth + 16
    implicitHeight: 20
    radius: 7
    color: active || mouse.containsMouse ? Theme.surface2 : "transparent"
    Behavior on color { ColorAnimation { duration: Theme.fastAnim } }

    Row {
        id: row
        // Centred on whole pixels, so the bar's glyphs stay sharp.
        x: Math.round((root.width - width) / 2)
        y: Math.round((root.height - height) / 2)
        spacing: root.spacing
    }

    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
