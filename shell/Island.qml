// The centre island hanging from the bar (Figma: "Center Island"), drawn
// with PanelShape so it reads as part of the bar, not a shape under it.
import QtQuick

Item {
    id: root
    signal clicked()

    width: shape.width
    height: shape.height

    PanelShape {
        id: shape
        bodyWidth: 170
        bodyHeight: 18
        earWidth: 12
        earHeight: 7
        bottomLeftRadius: 11
        bottomRightRadius: 11
    }

    // Grip: hints that the island opens the dock.
    Rectangle {
        anchors { horizontalCenter: parent.horizontalCenter; verticalCenter: parent.verticalCenter; verticalCenterOffset: 1 }
        width: 30; height: 3; radius: 2
        color: mouse.containsMouse ? Theme.muted : Theme.grip
        Behavior on color { ColorAnimation { duration: Theme.fastAnim } }
    }

    MouseArea {
        id: mouse
        // Only the body is clickable, not the ears.
        x: shape.bodyX
        width: shape.bodyWidth
        height: shape.bodyHeight
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
