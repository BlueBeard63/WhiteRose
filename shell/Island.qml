// The centre island (Figma: "Center Island"): one shape hanging from the bar
// that morphs between every view. The views are declared in shell/island/,
// one file each (IslandView.qml): the island takes each one's size, and
// what it asks for (the keyboard, room below). Switching views reshapes the
// same outline in place; closing shrinks it back to rest.
// When a window sits right under it (a maximized title bar), the idle island
// tucks into the bar, leaving its grip there; hovering brings it back out.
// Over a fullscreen window (no bar drawn) it rests hidden in the top edge:
// opening a view drops it out over the window, closing draws it back up.
import QtQuick
import Quickshell

Item {
    id: root

    // The switcher (run by the compositor) takes over while ALT+TAB is held.
    // A view for one screen only leaves the other islands at rest.
    readonly property string view: Compositor.switcher ? "switcher"
        : ShellState.islandScreen && ShellState.islandScreen !== screenName ? "idle" : ShellState.islandView
    readonly property bool idle: view === "idle"
    // Views that take the keyboard (each says; see IslandView.qml).
    readonly property bool wantsKeyboard: !idle && (shown?.keyboard ?? false)

    // Opening a switcher over another view ends that view.
    Connections {
        target: Compositor
        function onSwitcherChanged() { if (Compositor.switcher) ShellState.islandView = "idle" }
    }

    // ---- views -------------------------------------------------------------
    // The loader of the view showing, and the view itself.
    property var loaders: ({})
    readonly property var current: root.loaders[view] ?? null
    readonly property var shown: current?.item ?? null
    // ---- tucking ----------------------------------------------------------------
    // Hovering the bar's centre or the island itself keeps it out; leaving
    // tucks it back after a moment (so crossing the edge doesn't flicker).
    readonly property bool hovering: peek.containsMouse || gripMouse.containsMouse
    property bool peeking: false
    onHoveringChanged: {
        if (hovering) { unpeek.stop(); peeking = true; }
        else unpeek.restart();
    }
    Timer { id: unpeek; interval: 300; onTriggered: root.peeking = false }
    // The screen this island is on (its bar's), e.g. "DP-1".
    property string screenName: ""
    readonly property bool tucked: idle && Compositor.islandCovered.includes(screenName) && !peeking && !fullscreen
    // A fullscreen window on this screen (the bar sets it).
    property bool fullscreen: false
    // At rest out of sight: in the top edge over a fullscreen window.
    readonly property bool hidden: idle && fullscreen

    readonly property real targetWidth: idle || !current?.item ? 170 : current.item.implicitWidth
    readonly property real targetHeight: tucked || hidden ? 0 : idle || !current?.item ? 18 : current.item.implicitHeight

    // Room the bar window must leave below the bar. Grows at once, shrinks only
    // after the morph has finished, so the window never resizes mid-animation.
    readonly property real wanted: targetHeight + (root.shown?.overhang ?? 0)
    property real reserve: 18
    onWantedChanged: { if (wanted >= reserve) reserve = wanted; else shrink.restart() }
    Timer { id: shrink; interval: 320; onTriggered: root.reserve = root.wanted }

    // Things outside the outline that take clicks (the dock's previews): the
    // bar's input region includes them.
    readonly property var inputs: root.shown ? Array.from(root.shown.inputs) : []

    width: shape.width
    height: shape.height

    // ---- shape ---------------------------------------------------------------
    PanelShape {
        id: shape
        bodyWidth: root.targetWidth
        bodyHeight: root.targetHeight
        visible: bodyHeight > 0.5
        earWidth: root.tucked || root.hidden ? 0 : root.idle ? 12 : 26
        earHeight: root.tucked || root.hidden ? 0 : root.idle ? 7 : 30
        bottomLeftRadius: root.idle ? 11 : Math.min(40, Math.max(11, root.targetHeight - 30))
        bottomRightRadius: bottomLeftRadius

        Behavior on bodyWidth { NumberAnimation { duration: 280; easing.type: Easing.OutCubic } }
        Behavior on bodyHeight { NumberAnimation { duration: 280; easing.type: Easing.OutCubic } }
        Behavior on earWidth { NumberAnimation { duration: 280; easing.type: Easing.OutCubic } }
        Behavior on earHeight { NumberAnimation { duration: 280; easing.type: Easing.OutCubic } }
        Behavior on bottomLeftRadius { NumberAnimation { duration: 280; easing.type: Easing.OutCubic } }
    }
    // A new view waits until the outline has (almost) settled into its new
    // shape: at 200 of the 280 ms the OutCubic morph is ~98% done, so the
    // fade-in overlaps its tail. A view that only changes size (launcher
    // results while typing) stays visible, clipped to the outline as it
    // catches up, instead of blinking on every keystroke.
    readonly property bool morphing: settle.running
    Timer { id: settle; interval: 200 }
    onViewChanged: settle.restart()

    // Idle: the grip; clicking the island opens the dock.
    Rectangle {
        anchors { horizontalCenter: parent.horizontalCenter; top: parent.top; topMargin: 8 }
        width: 30; height: 3; radius: 2
        opacity: root.idle && !root.tucked && !root.hidden && !root.morphing ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: 120 } }
        color: gripMouse.containsMouse ? Theme.muted : Theme.grip
    }
    // Tucked: the grip sits in the bar, and the bar's centre is the handle.
    Rectangle {
        anchors.horizontalCenter: parent.horizontalCenter
        y: -Theme.barHeight / 2 - 1.5
        width: 30; height: 3; radius: 2
        opacity: root.tucked ? 1 : 0
        Behavior on opacity { NumberAnimation { duration: 160 } }
        color: peek.containsMouse ? Theme.muted : Theme.grip
    }
    MouseArea {
        id: peek
        visible: root.idle && !root.hidden
        x: (root.width - 170) / 2
        y: -Theme.barHeight
        width: 170
        height: Theme.barHeight
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: ShellState.show("dock")
    }
    MouseArea {
        id: gripMouse
        visible: root.idle && !root.hidden
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
        x: (parent.width - width) / 2
        y: 0
        active: selected || opacity > 0
        opacity: selected && !root.morphing ? 1 : 0
        visible: opacity > 0
        Behavior on opacity { NumberAnimation { duration: 110 } }
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
        // search field) still counts as the island having focus. They're
        // clipped to the outline, except one with room below it (the dock's
        // window previews hang there).
        Item {
            id: viewArea
            // A view that bleeds may also draw into the left ear: the clip
            // reaches into it, the views stay where they are.
            readonly property real bleed: root.shown?.bleed ? shape.bodyX : 0
            x: shape.bodyX - bleed
            width: shape.bodyWidth + bleed
            readonly property bool hangs: (root.shown?.overhang ?? 0) > 0
            height: hangs ? root.height + 400 : shape.bodyHeight
            clip: !hangs
            Item {
                x: viewArea.bleed
                width: shape.bodyWidth
                height: parent.height
                Repeater {
                    model: ShellState.islandViews
                    delegate: View {
                        id: viewLoader
                        required property string modelData
                        name: modelData
                        source: Quickshell.shellDir + "/island/" + modelData + ".qml"
                        Component.onCompleted: { const l = Object.assign({}, root.loaders); l[modelData] = viewLoader; root.loaders = l }
                        Component.onDestruction: { const l = Object.assign({}, root.loaders); delete l[modelData]; root.loaders = l }
                    }
                }
            }
        }

        onActiveFocusChanged: {
            if (activeFocus) focusSeen = true;
            else if (root.wantsKeyboard && focusSeen && !ShellState.dragging) ShellState.close();
        }
    }
    onWantsKeyboardChanged: if (!wantsKeyboard) keys.focusSeen = false
}
