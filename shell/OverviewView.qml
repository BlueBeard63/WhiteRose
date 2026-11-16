// The overview's shell part (Figma: "Overview - SUPER + W", "Overview -
// searching"). The compositor draws the windows spread out and the
// workspace row, and takes the pointer there (overview.rs); this draws
// over it, from the `overview` event: each spread window's icon and title,
// the close button on the one under the pointer (the compositor takes that
// click), and on the monitor it opened on the search. Typing searches the
// open windows first, then everything the launcher finds; arrows move,
// Enter opens. Esc is the compositor's (it closes the overview).
//
// The niri style (Figma "Overview niri - SUPER + W") has no search: only
// the hovered window's title, under its workspace (`title_y`), and its
// close button.
import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import "Calc.js" as Calc
import "Search.js" as Search

Scope {
    id: root
    readonly property var ov: Compositor.overview
    readonly property bool niri: ov?.style === "niri"
    property string query: ""
    property int selected: 0
    // Closed: the next one starts with an empty search.
    onOvChanged: if (!ov) { query = ""; selected = 0 }

    // ---- search (as the launcher's, plus the open windows) -----------------
    readonly property string home: Quickshell.env("HOME") ?? ""
    readonly property bool searching: query.trim() !== ""
    readonly property var apps: DesktopEntries.applications.values
        .filter(e => !e.noDisplay)
        .map(e => ({ id: e.id, name: e.name, genericName: e.genericName, keywords: e.keywords, comment: e.comment, entry: e }))
    property var recentFiles: []
    FileView {
        path: (Quickshell.env("XDG_DATA_HOME") || root.home + "/.local/share") + "/recently-used.xbel"
        watchChanges: true
        onFileChanged: reload()
        onLoaded: root.recentFiles = Search.parseRecent(text())
    }
    readonly property var items: !searching ? [] : Search.rankWindows(query, Compositor.windows, 5).concat(Search.results({
        query: query,
        category: "all",
        apps: apps,
        usage: LauncherConfig.launches,
        pinned: DockConfig.pinned,
        files: Daemon.setting("privacy.recentFiles", true) ? Activities.recentFiles(recentFiles) : [],
        commands: LauncherConfig.commands,
        history: LauncherConfig.history,
        calcHistory: LauncherConfig.calcHistory,
        calc: Calc.evaluate(query.trim()),
    }).items)
    onItemsChanged: selected = Math.min(selected, Math.max(0, items.length - 1))

    function activate(item) {
        if (!item) return;
        Compositor.setOverview(false);
        LauncherConfig.open(item);
    }

    Variants {
        model: root.ov ? Quickshell.screens.filter(s => root.ov.outputs.some(o => o.output === s.name)) : []
        delegate: PanelWindow {
            id: win
            required property var modelData
            screen: modelData
            readonly property var spread: root.ov?.outputs.find(o => o.output === modelData.name)?.windows ?? []
            readonly property bool hasSearch: root.ov?.search === modelData.name
            anchors { top: true; bottom: true; left: true; right: true }
            exclusionMode: ExclusionMode.Ignore
            color: "transparent"
            WlrLayershell.namespace: "whiterose-overview"
            LayerTitle { name: "whiterose-overview" }
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.keyboardFocus: hasSearch ? WlrKeyboardFocus.Exclusive : WlrKeyboardFocus.None
            // Only the search takes the pointer; the windows are the compositor's.
            mask: Region { item: !win.hasSearch ? nothing : root.searching ? backdrop : field }
            Item { id: nothing; width: 0; height: 0 }

            // Each spread window's icon and title; the close button on the
            // one under the pointer.
            Repeater {
                model: root.searching || root.ov?.dragging ? [] : win.spread
                delegate: Item {
                    id: spot
                    required property var modelData
                    readonly property var info: Compositor.windowById(modelData.id)
                    readonly property string title: info?.title || info?.app_id || ""
                    Accessible.role: Accessible.StaticText
                    Accessible.name: title
                    x: modelData.x; y: modelData.y
                    width: modelData.width; height: modelData.height
                    Row {
                        // GNOME: every title, under its window. niri: the
                        // hovered one's, under its workspace.
                        visible: !root.niri || root.ov?.hovered === spot.modelData.id
                        x: (spot.width - width) / 2
                        y: root.niri ? (spot.modelData.title_y ?? 0) - spot.y : spot.height + 12
                        spacing: 7
                        AppIcon {
                            anchors.verticalCenter: parent.verticalCenter
                            size: 15
                            entry: spot.info?.app_id ? DesktopEntries.heuristicLookup(spot.info.app_id) : null
                            name: spot.info?.app_id ?? ""
                        }
                        Text {
                            width: Math.min(implicitWidth, (root.niri ? Math.max(spot.width, 480) : spot.width) - 22)
                            elide: Text.ElideRight
                            text: spot.title
                            color: root.ov?.hovered === spot.modelData.id ? Theme.text : Theme.muted
                            font { family: Theme.font; pointSize: Theme.pt(12.5); weight: Font.Medium }
                        }
                    }
                    Rectangle {
                        visible: root.ov?.hovered === spot.modelData.id
                        x: parent.width - width / 2; y: -height / 2
                        width: 26; height: 26; radius: 13
                        color: Theme.surface2
                        Icon { anchors.centerIn: parent; name: "x"; size: 13; color: Theme.text }
                    }
                }
            }

            // Searching: the results cover the windows (not the bar).
            Rectangle {
                id: backdrop
                visible: win.hasSearch && root.searching
                y: field.y - 16
                width: win.width; height: win.height - y
                color: Qt.rgba(0.047, 0.047, 0.051, 1)
                // A click on nothing closes the overview, as the compositor's does.
                MouseArea { anchors.fill: parent; onClicked: Compositor.setOverview(false) }
            }

            // The search, on the monitor it opened on.
            Rectangle {
                id: field
                visible: win.hasSearch
                x: (win.width - width) / 2
                y: 46
                width: Math.min(592, win.width - 96)
                height: 46
                radius: 14
                color: Theme.surface1
                Row {
                    anchors { left: parent.left; leftMargin: 16; verticalCenter: parent.verticalCenter }
                    spacing: 10
                    Icon { anchors.verticalCenter: parent.verticalCenter; name: "search"; size: 18; color: Theme.muted }
                    Item {
                        anchors.verticalCenter: parent.verticalCenter
                        width: field.width - 16 - 18 - 10 - 16
                        height: 22
                        TextInput {
                            id: search
                            Accessible.role: Accessible.EditableText
                            Accessible.name: "Search"
                            anchors.fill: parent
                            verticalAlignment: TextInput.AlignVCenter
                            color: Theme.text
                            selectionColor: Theme.rose
                            selectedTextColor: Theme.bg
                            cursorDelegate: Rectangle { width: 1.5; color: Theme.rose; visible: search.cursorVisible }
                            font { family: Theme.font; pointSize: Theme.pt(14) }
                            focus: win.hasSearch
                            Component.onCompleted: if (win.hasSearch) forceActiveFocus()
                            text: root.query
                            onTextChanged: { root.query = text; root.selected = 0 }
                            Keys.onReturnPressed: root.activate(root.items[root.selected])
                            Keys.onEnterPressed: root.activate(root.items[root.selected])
                            Keys.onUpPressed: root.selected = Math.max(0, root.selected - 1)
                            Keys.onDownPressed: root.selected = Math.min(root.items.length - 1, root.selected + 1)
                        }
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: search.text === ""
                            text: "Search windows, apps, files and settings"
                            color: Theme.faint
                            font { family: Theme.font; pointSize: Theme.pt(14) }
                        }
                    }
                }
            }

            // Results under their headings: windows, apps, files...
            Column {
                id: results
                visible: win.hasSearch && root.searching
                x: field.x
                y: field.y + field.height + 22
                width: field.width
                spacing: 2
                Repeater {
                    model: root.items
                    delegate: Column {
                        id: entry
                        required property var modelData
                        required property int index
                        readonly property var it: modelData
                        readonly property string heading: Search.sectionOf(it.kind)
                        readonly property bool first: index === 0 || Search.sectionOf(root.items[index - 1].kind) !== heading
                        width: results.width
                        spacing: 6
                        Text {
                            visible: entry.first
                            topPadding: entry.index === 0 ? 0 : 8
                            text: entry.heading.toUpperCase()
                            color: Theme.faint
                            font { family: Theme.font; pointSize: Theme.pt(10.5); weight: Font.Medium; letterSpacing: 0.6 }
                        }
                        Rectangle {
                            id: row
                            readonly property bool sel: root.selected === entry.index
                            readonly property string title: ({
                                window: entry.it.window?.title || entry.it.window?.app_id,
                                app: entry.it.app?.name,
                                file: entry.it.file?.name,
                                calc: "= " + entry.it.result,
                                cmd: entry.it.cmd
                            })[entry.it.kind] ?? ""
                            readonly property string subtitle: ({
                                window: DesktopEntries.heuristicLookup(entry.it.window?.app_id ?? "")?.name ?? entry.it.window?.app_id ?? "",
                                app: entry.it.app?.genericName || entry.it.app?.comment || "",
                                file: Search.tildify(entry.it.file?.dir ?? "", root.home),
                                calc: entry.it.expr,
                                cmd: entry.it.terminal ? "Run in terminal" : entry.it.recent ? "Recent command" : "Run"
                            })[entry.it.kind] ?? ""
                            Accessible.role: Accessible.ListItem
                            Accessible.name: title
                            Accessible.description: subtitle
                            Accessible.selected: sel
                            Accessible.onPressAction: root.activate(entry.it)
                            width: parent.width
                            height: 46
                            radius: 12
                            color: sel || rowMouse.containsMouse ? Theme.surface1 : "transparent"
                            Row {
                                anchors { left: parent.left; leftMargin: 10; verticalCenter: parent.verticalCenter }
                                spacing: 12
                                Rectangle {
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: 30; height: 30; radius: 9
                                    color: row.sel ? Theme.surface2 : Theme.surface1
                                    AppIcon {
                                        anchors.centerIn: parent
                                        visible: entry.it.kind === "app" || entry.it.kind === "window"
                                        size: 20
                                        entry: entry.it.kind === "app" ? entry.it.app?.entry ?? null
                                            : DesktopEntries.heuristicLookup(entry.it.window?.app_id ?? "")
                                        name: row.title
                                    }
                                    Icon {
                                        anchors.centerIn: parent
                                        visible: entry.it.kind !== "app" && entry.it.kind !== "window"
                                        size: 15
                                        color: Theme.pearl
                                        name: entry.it.kind === "cmd" ? (entry.it.terminal ? "terminal" : entry.it.recent ? "restart" : "zap")
                                            : ({ file: "file", calc: "calculator" })[entry.it.kind] ?? ""
                                    }
                                }
                                Column {
                                    anchors.verticalCenter: parent.verticalCenter
                                    spacing: 1
                                    Text {
                                        width: results.width - 30 - 12 - 24 - workspaceText.width
                                        elide: Text.ElideRight
                                        text: row.title
                                        color: Theme.text
                                        font {
                                            family: entry.it.kind === "calc" || entry.it.kind === "cmd" ? Theme.mono : Theme.font
                                            pointSize: Theme.pt(13)
                                            weight: Font.Medium
                                        }
                                    }
                                    Text {
                                        width: results.width - 30 - 12 - 24 - workspaceText.width
                                        elide: Text.ElideMiddle
                                        visible: text !== ""
                                        text: row.subtitle
                                        color: Theme.muted
                                        font { family: entry.it.kind === "file" || entry.it.kind === "calc" ? Theme.mono : Theme.font; pointSize: Theme.pt(11) }
                                    }
                                }
                            }
                            // Where a window is.
                            Text {
                                id: workspaceText
                                anchors { right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter }
                                visible: entry.it.kind === "window"
                                width: visible ? implicitWidth : 0
                                text: "Workspace " + (entry.it.window?.workspace ?? "")
                                color: Theme.faint
                                font { family: Theme.font; pointSize: Theme.pt(11) }
                            }
                            MouseArea {
                                id: rowMouse
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onEntered: root.selected = entry.index
                                onClicked: root.activate(entry.it)
                            }
                        }
                    }
                }
                Text {
                    visible: root.items.length === 0
                    text: "No results"
                    color: Theme.faint
                    font { family: Theme.font; pointSize: Theme.pt(13) }
                }
            }
        }
    }
}
