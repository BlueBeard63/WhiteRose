// The centre island and the dock it opens into (Figma: Desktop - Idle ->
// Desktop - 1). One PanelShape morphs between the two, so the dock grows out
// of the bar instead of appearing on top of it.
import QtQuick
import Quickshell
import Quickshell.Wayland

Item {
    id: root
    readonly property bool open: ShellState.dockOpen
    // True from the moment it starts opening until it has finished closing;
    // the bar keeps its window tall for that long so it never resizes mid-animation.
    readonly property bool expanded: open || morph.running
    readonly property real openHeight: 87

    // ---- hover previews --------------------------------------------------
    // The running app whose windows are being previewed (after a short hover).
    property var previewApp: null
    readonly property bool previewing: open && previewApp !== null && previewWindows.length > 0
    readonly property real previewHeight: previewCard.implicitHeight
    readonly property alias previewCard: previewCard
    readonly property var previewWindows: previewApp
        ? ToplevelManager.toplevels.values.filter(t => entryIdFor(t.appId) === previewApp.key)
        : []
    onOpenChanged: if (!open) { previewApp = null; ShellState.dockPreviewKey = "" }
    Connections {
        target: ShellState
        function onDockPreviewKeyChanged() {
            const key = ShellState.dockPreviewKey;
            if (key) root.previewApp = root.apps.find(a => a.key === key && a.running) ?? null;
        }
    }

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

    // Hover intent: show previews after ~350ms on a running app, keep them
    // while the pointer is on the tile or the card, hide shortly after it leaves.
    readonly property var hoveredTile: {
        for (let i = 0; i < row.children.length; i++) {
            const c = row.children[i];
            if (c.hovered) return c;
        }
        return null;
    }
    readonly property bool pointerInPreview: previewHover.hovered

    // Like macOS: the first card needs a short hover; once one is open, moving
    // to another app switches immediately, and moving to an app with no
    // windows closes it.
    onHoveredTileChanged: {
        const tile = hoveredTile;
        if (!tile) return;                       // left the tiles: the hide timer decides
        ShellState.dockPreviewKey = "";          // a real hover overrides an IPC preview
        if (previewApp === null) return;         // not showing yet: wait for showPreview
        if (tile.app.key === previewApp.key) return;
        previewApp = tile.app.running ? tile.app : null;
    }
    Timer {
        id: showPreview
        interval: 350
        running: root.open && root.previewApp === null
                 && root.hoveredTile !== null && root.hoveredTile.app.running
        onTriggered: root.previewApp = root.hoveredTile?.app ?? null
    }
    // Pointer left both the tiles and the card (the delay bridges the gap
    // between a tile and its card).
    Timer {
        interval: 250
        running: root.previewApp !== null && root.hoveredTile === null && !root.pointerInPreview
                 && ShellState.dockPreviewKey === ""
        onTriggered: root.previewApp = null
    }

    Rectangle {
        id: previewCard
        visible: root.previewing
        readonly property var tile: {
            for (let i = 0; i < row.children.length; i++) {
                const c = row.children[i];
                if (c.app && root.previewApp && c.app.key === root.previewApp.key) return c;
            }
            return null;
        }
        // Centred under its tile, kept inside the dock window.
        x: tile ? Math.max(-root.x + 8, row.x + tile.x + tile.width / 2 - width / 2) : 0
        y: shape.height + 8
        implicitWidth: previews.implicitWidth + 24
        implicitHeight: previews.implicitHeight + 24
        width: implicitWidth
        height: implicitHeight
        radius: 18
        color: Theme.bg
        HoverHandler { id: previewHover }

        Row {
            id: previews
            anchors.centerIn: parent
            spacing: 12
            Repeater {
                model: root.previewWindows
                delegate: WindowPreview {
                    required property var modelData
                    toplevel: modelData
                    onActivated: { modelData.activate(); root.close(); }
                }
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
        visible: root.open && hovered !== null && !root.previewing
        x: hovered ? row.x + hovered.x + hovered.width / 2 - width / 2 : 0
        y: shape.height + 6
        width: tipText.implicitWidth + 16
        height: 22
        radius: 7
        color: Theme.bg
        Text {
            id: tipText
            anchors.centerIn: parent
            text: tip.hovered ? (tip.hovered.app.entry?.name ?? tip.hovered.app.appId) : ""
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
