// Island view: the app launcher (Figma: App Launcher - SUPER). Tap SUPER or
// the dock's launcher tile. Type to search apps, recent files, a calculator,
// or run a command with ">". Arrows move, Enter opens, Esc closes.
import QtQuick
import Quickshell
import Quickshell.Io
import "Calc.js" as Calc
import "Search.js" as Search

Item {
    id: root
    implicitWidth: 640
    implicitHeight: content.height + 40

    // Set once the search field has had focus; losing it afterwards closes.
    property bool focusSeen: false
    function close() { ShellState.close() }
    Component.onCompleted: {
        search.text = ShellState.launcherQuery;
        ShellState.launcherQuery = "";
        search.forceActiveFocus();
    }
    Connections {
        target: ShellState
        function onLauncherQueryChanged() {
            if (ShellState.launcherQuery !== "") {
                search.text = ShellState.launcherQuery;
                ShellState.launcherQuery = "";
            }
        }
    }

    // ---- data --------------------------------------------------------------
    property string query: ""
    property string category: "all"    // all | apps | files | commands | calculator
    property int selected: 0
    readonly property string home: Quickshell.env("HOME") ?? ""

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

    readonly property bool browsing: query.trim() === ""
    readonly property var gridApps: Search.frequent(apps, LauncherConfig.launches, DockConfig.pinned, 12)

    // Every selectable item, in display order.
    readonly property var items: {
        const q = query.trim();
        if (q === "") {
            const grid = gridApps.map(a => ({ kind: "app", app: a }));
            return grid.concat(recentFiles.slice(0, 3).map(f => ({ kind: "file", file: f })));
        }
        if (q.startsWith(">") || category === "commands") {
            const cmd = q.replace(/^>\s*/, "");
            return cmd ? [{ kind: "cmd", cmd }] : [];
        }
        const out = [];
        const calc = Calc.evaluate(q);
        if (calc !== null && (category === "all" || category === "calculator")) out.push({ kind: "calc", expr: q, result: calc });
        if (category === "calculator") return out;
        if (category === "all" || category === "apps")
            for (const r of Search.rankApps(q, apps, LauncherConfig.launches, 5)) out.push({ kind: "app", app: r.app });
        if (category === "all" || category === "files")
            for (const r of Search.matchFiles(q, recentFiles, 3)) out.push({ kind: "file", file: r.file });
        return out;
    }
    onItemsChanged: selected = Math.min(selected, Math.max(0, items.length - 1))
    readonly property int gridCount: browsing ? gridApps.length : 0

    function activate(item) {
        if (!item) return;
        switch (item.kind) {
        case "app": item.app.entry.execute(); LauncherConfig.record(item.app.id); break;
        case "file": Quickshell.execDetached(["xdg-open", item.file.path]); break;
        case "calc": Quickshell.clipboardText = item.result; break;
        case "cmd": Quickshell.execDetached(["sh", "-c", item.cmd]); break;
        }
        close();
    }

    // Arrow keys: a 6-wide grid while browsing, a list while searching.
    function move(dx, dy) {
        const n = items.length;
        if (!n) return;
        let i = selected;
        if (browsing && i < gridCount) {
            if (dx) i = Math.max(0, Math.min(gridCount - 1, i + dx));
            if (dy > 0) i = i + 6 < gridCount ? i + 6 : (gridCount < n ? gridCount : i);
            if (dy < 0) i = Math.max(i % 6 === i ? i : i - 6, 0);
        } else {
            const d = dy || dx;
            i += d;
            if (browsing && i < gridCount) i = Math.max(0, gridCount - 1);
        }
        selected = Math.max(0, Math.min(n - 1, i));
    }

    // ---- view --------------------------------------------------------------
    Column {
        id: content
        x: 24
        y: 18
        width: root.implicitWidth - 48
        spacing: 14

        // Search field.
        Rectangle {
            width: parent.width
            height: 46
            radius: 14
            color: Theme.surface1
            Row {
                anchors { left: parent.left; leftMargin: 16; verticalCenter: parent.verticalCenter }
                spacing: 10
                Icon { anchors.verticalCenter: parent.verticalCenter; name: "search"; size: 18; color: Theme.muted }
                Item {
                    anchors.verticalCenter: parent.verticalCenter
                    width: content.width - 16 - 18 - 10 - 16
                    height: 22
                    TextInput {
                        id: search
                        anchors.fill: parent
                        verticalAlignment: TextInput.AlignVCenter
                        color: Theme.text
                        selectionColor: Theme.rose
                        selectedTextColor: Theme.bg
                        cursorDelegate: Rectangle { width: 1.5; color: Theme.rose; visible: search.cursorVisible }
                        font { family: Theme.font; pointSize: Theme.pt(14) }
                        focus: true
                        onTextChanged: { root.query = text; root.selected = 0; }
                        onActiveFocusChanged: {
                            if (activeFocus) root.focusSeen = true;
                            // Clicking a window or the desktop takes focus away: close.
                            else if (root.focusSeen && ShellState.islandView === "launcher") root.close();
                        }
                        Keys.onEscapePressed: root.close()
                        Keys.onReturnPressed: root.activate(root.items[root.selected])
                        Keys.onEnterPressed: root.activate(root.items[root.selected])
                        Keys.onUpPressed: root.move(0, -1)
                        Keys.onDownPressed: root.move(0, 1)
                        Keys.onLeftPressed: event => { if (root.browsing) root.move(-1, 0); else event.accepted = false; }
                        Keys.onRightPressed: event => { if (root.browsing) root.move(1, 0); else event.accepted = false; }
                        Keys.onTabPressed: {
                            const cats = ["all", "apps", "files", "commands", "calculator"];
                            root.category = cats[(cats.indexOf(root.category) + 1) % cats.length];
                        }
                    }
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: search.text === ""
                        text: "Search apps, files and commands"
                        color: Theme.faint
                        font { family: Theme.font; pointSize: Theme.pt(14) }
                    }
                }
            }
        }

        // Category chips.
        Row {
            spacing: 6
            Repeater {
                model: [["all", "All"], ["apps", "Apps"], ["files", "Files"], ["commands", "Commands"], ["calculator", "Calculator"]]
                delegate: Rectangle {
                    required property var modelData
                    readonly property bool active: root.category === modelData[0]
                    width: chipText.implicitWidth + 24; height: 26; radius: 13
                    color: active ? Theme.pearl : Theme.surface1
                    Text {
                        id: chipText
                        anchors.centerIn: parent
                        text: modelData[1]
                        color: parent.active ? Theme.bg : Theme.muted
                        font { family: Theme.font; pointSize: Theme.pt(12); weight: Font.Medium }
                    }
                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: { root.category = modelData[0]; search.forceActiveFocus(); }
                    }
                }
            }
        }

        // Browsing: most-used apps as a grid.
        Grid {
            visible: root.browsing && root.gridCount > 0
            columns: 6
            columnSpacing: 8
            rowSpacing: 4
            Repeater {
                model: root.browsing ? root.gridApps : []
                delegate: Rectangle {
                    id: cell
                    required property var modelData
                    required property int index
                    readonly property bool sel: root.selected === index
                    width: (content.width - 5 * 8) / 6
                    height: 94
                    radius: 16
                    color: sel || cellMouse.containsMouse ? Theme.surface1 : "transparent"
                    Column {
                        anchors.centerIn: parent
                        spacing: 8
                        Rectangle {
                            anchors.horizontalCenter: parent.horizontalCenter
                            width: 52; height: 52; radius: 15
                            color: cell.sel ? Theme.surface2 : Theme.surface1
                            AppIcon { anchors.centerIn: parent; size: 30; entry: cell.modelData.entry; name: cell.modelData.name }
                        }
                        Text {
                            anchors.horizontalCenter: parent.horizontalCenter
                            width: cell.width - 8
                            horizontalAlignment: Text.AlignHCenter
                            elide: Text.ElideRight
                            text: cell.modelData.name
                            color: Theme.text
                            font { family: Theme.font; pointSize: Theme.pt(11.5); weight: Font.Medium }
                        }
                    }
                    MouseArea {
                        id: cellMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onEntered: root.selected = cell.index
                        onClicked: root.activate({ kind: "app", app: cell.modelData })
                    }
                }
            }
        }

        // Searching (and recent files while browsing): rows.
        Column {
            width: parent.width
            spacing: 2
            Repeater {
                model: root.items.slice(root.gridCount)
                delegate: Rectangle {
                    id: row
                    required property var modelData
                    required property int index
                    readonly property int itemIndex: root.gridCount + index
                    readonly property bool sel: root.selected === itemIndex
                    readonly property var it: modelData
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
                                visible: row.it.kind === "app"
                                size: 20
                                entry: row.it.app?.entry ?? null
                                name: row.it.app?.name ?? ""
                            }
                            Icon {
                                anchors.centerIn: parent
                                visible: row.it.kind !== "app"
                                size: 15
                                color: Theme.pearl
                                name: ({ file: "file", calc: "calculator", cmd: "terminal" })[row.it.kind] ?? ""
                            }
                        }
                        Column {
                            anchors.verticalCenter: parent.verticalCenter
                            spacing: 1
                            Text {
                                width: content.width - 30 - 12 - 24
                                elide: Text.ElideRight
                                text: ({
                                    app: row.it.app?.name,
                                    file: row.it.file?.name,
                                    calc: "= " + row.it.result,
                                    cmd: row.it.cmd
                                })[row.it.kind] ?? ""
                                color: Theme.text
                                font {
                                    family: row.it.kind === "calc" || row.it.kind === "cmd" ? Theme.mono : Theme.font
                                    pointSize: Theme.pt(13)
                                    weight: Font.Medium
                                }
                            }
                            Text {
                                width: content.width - 30 - 12 - 24
                                elide: Text.ElideMiddle
                                visible: text !== ""
                                text: ({
                                    app: row.it.app?.genericName || row.it.app?.comment || "",
                                    file: Search.tildify(row.it.file?.dir ?? "", root.home),
                                    calc: row.it.expr,
                                    cmd: "Run in a shell"
                                })[row.it.kind] ?? ""
                                color: Theme.muted
                                font { family: row.it.kind === "file" || row.it.kind === "calc" ? Theme.mono : Theme.font; pointSize: Theme.pt(11) }
                            }
                        }
                    }
                    MouseArea {
                        id: rowMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onEntered: root.selected = row.itemIndex
                        onClicked: root.activate(row.it)
                    }
                }
            }
        }

        // Nothing matched.
        Text {
            visible: !root.browsing && root.items.length === 0
            text: "No results"
            color: Theme.faint
            font { family: Theme.font; pointSize: Theme.pt(13) }
        }
    }
}
