// The centre island (Figma: "Center Island"): one shape hanging from the bar
// that morphs between every view: idle, dock, clipboard, screenshots,
// launcher, and the ALT+TAB / SUPER+ALT+TAB switchers. Switching views
// reshapes the same outline in place; closing shrinks it back to idle.
import QtQuick
import Quickshell

Item {
    id: root

    // The switcher (run by the compositor) takes over while ALT+TAB is held.
    readonly property string view: Compositor.switcher ? "switcher" : ShellState.islandView
    readonly property bool idle: view === "idle"
    // Views that take the keyboard (the switcher doesn't: the compositor owns
    // those keys).
    readonly property bool wantsKeyboard: !idle && view !== "switcher"

    // Opening a switcher over another view ends that view.
    Connections {
        target: Compositor
        function onSwitcherChanged() { if (Compositor.switcher) ShellState.islandView = "idle" }
    }

    // ---- views -------------------------------------------------------------
    readonly property var current: ({
        dock: dockLoader, clipboard: clipLoader, screenshots: shotsLoader,
        launcher: launcherLoader, switcher: switcherLoader
    })[view] ?? null
    readonly property real targetWidth: idle || !current?.item ? 170 : current.item.implicitWidth
    readonly property real targetHeight: idle || !current?.item ? 18 : current.item.implicitHeight

    // Room the bar window must leave below the bar. Grows at once, shrinks only
    // after the morph has finished, so the window never resizes mid-animation.
    readonly property real wanted: targetHeight + (view === "dock" && dockLoader.item ? dockLoader.item.overhang : 0)
    property real reserve: 18
    onWantedChanged: { if (wanted >= reserve) reserve = wanted; else shrink.restart() }
    Timer { id: shrink; interval: 320; onTriggered: root.reserve = root.wanted }

    readonly property var previewCard: dockLoader.item?.previewCard ?? null

    width: shape.width
    height: shape.height

    // ---- shape ---------------------------------------------------------------
    PanelShape {
        id: shape
        bodyWidth: root.targetWidth
        bodyHeight: root.targetHeight
        earWidth: root.idle ? 12 : 26
        earHeight: root.idle ? 7 : 30
        bottomLeftRadius: root.idle ? 11 : Math.min(40, Math.max(11, root.targetHeight - 30))
        bottomRightRadius: bottomLeftRadius

        Behavior on bodyWidth { NumberAnimation { duration: 280; easing.type: Easing.OutCubic } }
        Behavior on bodyHeight { NumberAnimation { duration: 280; easing.type: Easing.OutCubic } }
        Behavior on earWidth { NumberAnimation { duration: 280; easing.type: Easing.OutCubic } }
        Behavior on earHeight { NumberAnimation { duration: 280; easing.type: Easing.OutCubic } }
        Behavior on bottomLeftRadius { NumberAnimation { duration: 280; easing.type: Easing.OutCubic } }
    }
    // Content waits until the outline has settled into its new shape (any
    // dimension may change, not just the width).
    readonly property bool morphing: settle.running
    Timer { id: settle; interval: 280 }
    onTargetWidthChanged: settle.restart()
    onTargetHeightChanged: settle.restart()

    // Idle: the grip; clicking the island opens the dock.
    Rectangle {
        anchors { horizontalCenter: parent.horizontalCenter; top: parent.top; topMargin: 8 }
        width: 30; height: 3; radius: 2
        opacity: root.idle && !root.morphing ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: 120 } }
        color: gripMouse.containsMouse ? Theme.muted : Theme.grip
    }
    MouseArea {
        id: gripMouse
        visible: root.idle
        x: shape.bodyX
        width: shape.bodyWidth
        height: shape.bodyHeight
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: ShellState.show("dock")
    }

    // Each view is loaded while it's shown (and while it fades out), centred
    // in the island, and fades in once the shape has room for it.
    component View: Loader {
        required property string name
        readonly property bool selected: root.view === name
        x: (root.width - width) / 2
        y: 0
        active: selected || opacity > 0
        opacity: selected && !root.morphing ? 1 : 0
        visible: opacity > 0
        Behavior on opacity { NumberAnimation { duration: 130 } }
    }
    // ---- keyboard ------------------------------------------------------------
    // Esc closes; losing focus (clicking a window or the desktop) closes.
    FocusScope {
        id: keys
        anchors.fill: parent
        focus: root.wantsKeyboard
        property bool focusSeen: false
        Keys.onEscapePressed: ShellState.close()

        // Views live inside the scope, so a focused child (the launcher's
        // search field) still counts as the island having focus.
        View { id: dockLoader; name: "dock"; sourceComponent: DockView {} }
        View { id: clipLoader; name: "clipboard"; sourceComponent: ClipboardView {} }
        View { id: shotsLoader; name: "screenshots"; sourceComponent: ScreenshotsView {} }
        View { id: launcherLoader; name: "launcher"; sourceComponent: LauncherView {} }
        View { id: switcherLoader; name: "switcher"; sourceComponent: SwitcherView {} }

        onActiveFocusChanged: {
            if (activeFocus) focusSeen = true;
            else if (root.wantsKeyboard && focusSeen && !ShellState.dragging) ShellState.close();
        }
    }
    onWantsKeyboardChanged: if (!wantsKeyboard) keys.focusSeen = false
}
