// The centre island and the dock it opens into (Figma: Desktop - Idle ->
// Desktop - 1). One PanelShape morphs between the two, so the dock grows out
// of the bar instead of appearing on top of it.
import QtQuick
import Quickshell

Item {
    id: root
    readonly property bool open: ShellState.dockOpen
    // True from the moment it starts opening until it has finished closing;
    // the bar keeps its window tall for that long so it never resizes mid-animation.
    readonly property bool expanded: open || morph.running
    readonly property real openHeight: 87

    function toggle() { ShellState.dockOpen = !ShellState.dockOpen }
    function close() { ShellState.dockOpen = false }

    width: shape.width
    height: shape.height

    // ---- apps ------------------------------------------------------------
    // A window's app id (e.g. "firefox") mapped to its desktop entry id
    // (e.g. "firefox_firefox" for the snap), so pins and windows line up.
    function entryIdFor(appId) {
        const e = appId ? DesktopEntries.heuristicLookup(appId) : null;
        return e ? e.id : (appId ?? "");
    }

    readonly property var apps: {
        const wins = Compositor.windows;
        const byKey = {};
        for (const w of wins) {
            const key = entryIdFor(w.app_id);
            if (!key) continue;
            const a = byKey[key] ?? (byKey[key] = { appId: w.app_id, running: true, focused: false });
            a.focused = a.focused || w.focused;
        }
        const list = [];
        for (const key of DockConfig.pinned) {
            const entry = DesktopEntries.byId(key);
            if (!entry) continue;   // not installed
            const run = byKey[key];
            list.push({ key, entry, appId: run?.appId ?? "", running: !!run, focused: run?.focused ?? false, pinned: true });
        }
        // Running apps that aren't pinned, most recently used first.
        for (const w of wins) {
            const key = entryIdFor(w.app_id);
            if (!key || list.some(a => a.key === key)) continue;
            const run = byKey[key];
            list.push({ key, entry: DesktopEntries.byId(key) ?? DesktopEntries.heuristicLookup(w.app_id),
                        appId: run.appId, running: true, focused: run.focused, pinned: false });
        }
        return list;
    }

    function activate(app) {
        if (app.running) Compositor.focusApp(app.appId);
        else app.entry?.execute();
        close();
    }

    // ---- shape -----------------------------------------------------------
    PanelShape {
        id: shape
        // Idle island vs open dock; every number animates together.
        bodyWidth: root.open ? row.implicitWidth + 2 * 24 : 170
        bodyHeight: root.open ? root.openHeight : 18
        earWidth: root.open ? 26 : 12
        earHeight: root.open ? 30 : 7
        bottomLeftRadius: root.open ? 40 : 11
        bottomRightRadius: root.open ? 40 : 11

        Behavior on bodyWidth { NumberAnimation { id: morph; duration: 260; easing.type: Easing.OutCubic } }
        Behavior on bodyHeight { NumberAnimation { duration: 260; easing.type: Easing.OutCubic } }
        Behavior on earWidth { NumberAnimation { duration: 260; easing.type: Easing.OutCubic } }
        Behavior on earHeight { NumberAnimation { duration: 260; easing.type: Easing.OutCubic } }
        Behavior on bottomLeftRadius { NumberAnimation { duration: 260; easing.type: Easing.OutCubic } }
        Behavior on bottomRightRadius { NumberAnimation { duration: 260; easing.type: Easing.OutCubic } }
    }

    // Idle: the grip. Clicking the island opens the dock.
    Rectangle {
        anchors { horizontalCenter: parent.horizontalCenter; top: parent.top; topMargin: 8 }
        width: 30; height: 3; radius: 2
        opacity: root.open ? 0 : 1
        Behavior on opacity { NumberAnimation { duration: 120 } }
        color: islandMouse.containsMouse ? Theme.muted : Theme.grip
    }
    MouseArea {
        id: islandMouse
        visible: !root.open
        x: shape.bodyX
        width: shape.bodyWidth
        height: shape.bodyHeight
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.toggle()
    }

    // Open: the apps. Fades in once the shape has room for it.
    Row {
        id: row
        anchors { horizontalCenter: parent.horizontalCenter; top: parent.top; topMargin: 18 }
        spacing: 10
        opacity: root.open && !morph.running ? 1 : 0
        visible: opacity > 0
        Behavior on opacity { NumberAnimation { duration: 140 } }

        Repeater {
            model: root.apps
            delegate: DockTile {
                required property var modelData
                app: modelData
                onActivated: root.activate(modelData)
                onNewWindow: { modelData.entry?.execute(); root.close(); }
                onTogglePin: DockConfig.togglePin(modelData.key)
            }
        }
    }

    // App name under the dock while hovering a tile.
    Rectangle {
        id: tip
        readonly property var hovered: {
            for (let i = 0; i < row.children.length; i++) {
                const c = row.children[i];
                if (c.hovered) return c;
            }
            return null;
        }
        visible: root.open && hovered !== null
        x: hovered ? row.x + hovered.x + hovered.width / 2 - width / 2 : 0
        y: shape.height + 6
        width: tipText.implicitWidth + 16
        height: 22
        radius: 7
        color: Theme.bg
        Text {
            id: tipText
            anchors.centerIn: parent
            text: tip.hovered ? (tip.hovered.app.entry?.name ?? tip.hovered.app.appId) + (tip.hovered.app.pinned ? "" : "  ·  right-click to pin") : ""
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(11.5); weight: Font.Medium }
        }
    }

    // Keyboard: Esc closes; losing focus (clicking a window or the desktop) closes.
    FocusScope {
        id: keys
        anchors.fill: parent
        focus: root.open
        Keys.onEscapePressed: root.close()
        onActiveFocusChanged: if (!activeFocus && root.open && focusSeen) root.close()
        property bool focusSeen: false
        onFocusChanged: if (!focus) focusSeen = false
    }
    Connections {
        target: keys
        function onActiveFocusChanged() { if (keys.activeFocus) keys.focusSeen = true }
    }
}
