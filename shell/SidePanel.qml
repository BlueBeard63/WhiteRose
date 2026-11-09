// A panel hanging from the right end of the bar (quick settings,
// notifications): one shape joined to the bar by an ear on its left, that
// grows down when opened and shrinks back up, with the content fading in once
// it has room. Esc or clicking elsewhere closes it. Children go in a 324 px
// column.
import QtQuick

Item {
    id: root

    // ShellState.panel value that opens this panel (or set `open` directly).
    property string name: ""
    property bool open: name !== "" && ShellState.panel === name
    // Panels you open take the keyboard (Esc, closing on focus loss); popups don't.
    property bool takesKeyboard: true
    default property alias content: column.data
    property alias spacing: column.spacing
    // Called on Esc; return true to keep the panel open (e.g. a sub-menu closed).
    property var escapeHandler: null

    // The column plus its top and bottom padding.
    readonly property real targetHeight: open ? column.implicitHeight + 16 + 20 : 0
    // Room the bar window must leave below the bar (grows at once, shrinks
    // after the closing animation), as for the island.
    property real reserve: 0
    onTargetHeightChanged: { if (targetHeight >= reserve) reserve = targetHeight; else shrink.restart() }
    Timer { id: shrink; interval: 320; onTriggered: root.reserve = root.targetHeight }

    width: shape.width
    height: shape.height
    visible: shape.bodyHeight > 0.5

    PanelShape {
        id: shape
        bodyWidth: 360
        bodyHeight: root.targetHeight
        rightEar: false
        earWidth: 49
        earHeight: Math.min(55, bodyHeight)
        bottomLeftRadius: 32
        bottomRightRadius: 0
        Behavior on bodyHeight { NumberAnimation { duration: 280; easing.type: Easing.OutCubic } }
    }

    // Content appears once the outline has (almost) finished growing.
    Timer { id: settle; interval: 200 }
    onOpenChanged: if (open) settle.restart()

    FocusScope {
        anchors.fill: parent
        focus: root.open && root.takesKeyboard
        property bool focusSeen: false
        Keys.onEscapePressed: { if (!(root.escapeHandler && root.escapeHandler())) ShellState.closePanel() }
        onActiveFocusChanged: {
            if (activeFocus) focusSeen = true;
            else if (root.open && root.takesKeyboard && focusSeen) ShellState.closePanel();
        }

        Column {
            id: column
            x: shape.bodyX + 18
            y: 16
            width: 324
            spacing: 12
            opacity: root.open && !settle.running ? 1 : 0
            visible: opacity > 0
            Behavior on opacity { NumberAnimation { duration: 110 } }
        }
    }
}
