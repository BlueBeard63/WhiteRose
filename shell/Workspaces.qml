// Workspace dots (Figma: Workspaces). Active = wide pearl pill, occupied =
// muted dot, empty = faint dot. Driven by compositor IPC events.
import QtQuick

Row {
    id: root
    spacing: 5
    leftPadding: 6
    rightPadding: 6

    // Show at least 5 dots, more if a higher workspace is in use.
    readonly property int shown: Math.max(5, Compositor.activeWorkspace,
        ...(Compositor.occupiedWorkspaces.length ? Compositor.occupiedWorkspaces : [0]))

    Repeater {
        model: root.shown
        delegate: Rectangle {
            required property int index
            readonly property int number: index + 1
            readonly property bool isActive: Compositor.activeWorkspace === number
            readonly property bool occupied: Compositor.occupiedWorkspaces.includes(number)

            anchors.verticalCenter: parent.verticalCenter
            width: isActive ? 18 : 6
            height: 6
            radius: 3
            color: isActive ? Theme.pearl : occupied ? Theme.muted : Theme.faint
            Behavior on width { NumberAnimation { duration: Theme.fastAnim; easing.type: Easing.OutCubic } }
            Behavior on color { ColorAnimation { duration: Theme.fastAnim } }

            MouseArea {
                anchors.fill: parent
                anchors.margins: -4   // easier to hit than 6px
                cursorShape: Qt.PointingHandCursor
                onClicked: Compositor.workspace(parent.number)
            }
        }
    }
}
