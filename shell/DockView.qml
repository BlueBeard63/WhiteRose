// Island view: the dock (Figma: Desktop - 1). Launcher tile, pinned and
// running apps, then buttons for the clipboard and screenshot views. Hovering
// a running app shows live previews of its windows below the island.
import QtQuick
import Quickshell
import Quickshell.Wayland

Item {
    id: root
    implicitWidth: row.implicitWidth + 48
    implicitHeight: 87

    // Extra room below the island for the preview card or the name tip.
    readonly property real overhang: previewing ? previewCard.height + 16 : 34
    readonly property alias previewCard: previewCard

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
        ShellState.close();
    }

    Row {
        id: row
        anchors { horizontalCenter: parent.horizontalCenter; top: parent.top; topMargin: 18 }
        spacing: 10

        // Launcher tile (Figma: pearl tile with a grid).
        Rectangle {
            width: 42; height: 42; radius: 13
            color: launcherMouse.containsMouse ? Qt.lighter(Theme.pearl, 1.04) : Theme.pearl
            scale: launcherMouse.pressed ? 0.92 : 1
            Behavior on scale { NumberAnimation { duration: 90 } }
            Icon { anchors.centerIn: parent; name: "grid"; size: 19; color: Theme.bg }
            MouseArea {
                id: launcherMouse
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: ShellState.show("launcher")
            }
        }
        Item { width: 1; height: 42; Rectangle { anchors.centerIn: parent; width: 1; height: 24; color: Theme.surface2 } }

        Repeater {
            model: root.apps
            delegate: DockTile {
                required property var modelData
                app: modelData
                onActivated: root.activate(modelData)
                onNewWindow: { modelData.entry?.execute(); ShellState.close(); }
                onTogglePin: DockConfig.togglePin(modelData.key)
            }
        }

        // Clipboard and screenshots: the island morphs into those views.
        Item { width: 1; height: 42; Rectangle { anchors.centerIn: parent; width: 1; height: 24; color: Theme.surface2 } }
        Column {
            spacing: 6
            IconButton { width: 42; height: 18; radius: 7; icon: "clipboard"; iconSize: 12; onClicked: ShellState.show("clipboard") }
            IconButton { width: 42; height: 18; radius: 7; icon: "image"; iconSize: 12; onClicked: ShellState.show("screenshots") }
        }
    }

    // ---- hover previews --------------------------------------------------
    property var previewApp: null
    readonly property bool previewing: previewApp !== null && previewWindows.length > 0
    readonly property var previewWindows: previewApp
        ? ToplevelManager.toplevels.values.filter(t => entryIdFor(t.appId) === previewApp.key)
        : []
    Component.onCompleted: if (ShellState.dockPreviewKey) applyPreviewKey()
    function applyPreviewKey() {
        const key = ShellState.dockPreviewKey;
        if (key) previewApp = apps.find(a => a.key === key && a.running) ?? null;
    }
    Connections {
        target: ShellState
        function onDockPreviewKeyChanged() { root.applyPreviewKey() }
    }

    readonly property var hoveredTile: {
        for (let i = 0; i < row.children.length; i++) {
            const c = row.children[i];
            if (c.hovered && c.app) return c;
        }
        return null;
    }
    // Like macOS: the first card needs a short hover; once one is open, moving
    // to another app switches immediately, and moving to an app with no
    // windows closes it.
    onHoveredTileChanged: {
        const tile = hoveredTile;
        if (!tile) return;
        ShellState.dockPreviewKey = "";
        if (previewApp === null) return;
        if (tile.app.key === previewApp.key) return;
        previewApp = tile.app.running ? tile.app : null;
    }
    Timer {
        interval: 350
        running: root.previewApp === null && root.hoveredTile !== null && root.hoveredTile.app.running
        onTriggered: root.previewApp = root.hoveredTile?.app ?? null
    }
    Timer {
        interval: 250
        running: root.previewApp !== null && root.hoveredTile === null && !previewHover.hovered && ShellState.dockPreviewKey === ""
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
        x: tile ? row.x + tile.x + tile.width / 2 - width / 2 : 0
        y: root.implicitHeight + 8
        width: previews.implicitWidth + 24
        height: previews.implicitHeight + 24
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
                    onActivated: { modelData.activate(); ShellState.close(); }
                }
            }
        }
    }

    // App name below the island while hovering a tile.
    Rectangle {
        readonly property var tile: root.hoveredTile
        visible: tile !== null && !root.previewing
        x: tile ? row.x + tile.x + tile.width / 2 - width / 2 : 0
        y: root.implicitHeight + 6
        width: tipText.implicitWidth + 16
        height: 22
        radius: 7
        color: Theme.bg
        Text {
            id: tipText
            anchors.centerIn: parent
            text: parent.tile ? (parent.tile.app.entry?.name ?? parent.tile.app.appId) : ""
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(11.5); weight: Font.Medium }
        }
    }
}
