// Details: name and the chosen columns (size, modified, created, kind).
// Click a heading to sort by it (again to reverse), drag its left edge to
// size it, right-click for which columns. Grouped: headings between groups.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F

FocusScope {
    id: root
    required property var pane
    readonly property real now: Date.now()
    // In the trash and in search results the last column says where things are.
    readonly property bool located: root.pane.inTrash || root.pane.searching
    readonly property bool renamingNow: Object.keys(root.pane.renaming).length > 0
    // Every column there can be after Name: what it's called, how wide, what
    // it shows, and what sorting by it means ("": it can't sort).
    readonly property var all: ({
        size: { label: "Size", w: 90, sort: "size", value: e => !e.dir ? F.size(e.size) : root.pane.sizes[e.path] ? F.size(root.pane.sizes[e.path].bytes) : root.pane.wantsSizes && !e.link ? "…" : "" },
        modified: { label: root.pane.inTrash ? "Deleted" : "Modified", w: 150, sort: "modified", value: e => F.date(e.modified, root.now) },
        created: { label: "Created", w: 150, sort: "created", value: e => e.created ? F.date(e.created, root.now) : "" },
        kind: { label: "Kind", w: 110, sort: "kind", value: e => F.kind(e.mime) },
        // Coloured dots and names.
        tags: { label: "Tags", w: 160, sort: "", rich: true,
                value: e => (e.tags ?? []).map(t => '<font color="' + Session.tagColour(t) + '">●</font>&nbsp;' + t.replace(/&/g, "&amp;").replace(/</g, "&lt;")).join("&nbsp;&nbsp;") },
        where: { label: root.pane.inTrash ? "From" : "Folder", w: 220, sort: "", value: e => String(e.origin ?? e.folder ?? "").replace(Fs.home(), "~") },
    })
    // The columns shown (the trash and search: their own), widths as set,
    // dropped from the end while the name would get under 160 px.
    readonly property var cols: {
        // Renaming in the Sheet: room for "old → new", with only the date beside.
        const keys = root.renamingNow ? ["modified"] : root.located ? ["size", "modified", "where"] : Session.columns;
        const list = keys.filter(k => root.all[k]).map(k => Object.assign({ key: k }, root.all[k], { w: Session.widths[k] ?? root.all[k].w }));
        while (list.length > 0 && root.width - 32 - list.reduce((a, c) => a + c.w, 0) < 160) list.pop();
        return list;
    }
    readonly property real nameW: root.width - 32 - root.cols.reduce((a, c) => a + c.w, 0)

    // ---- headings (click: sort; drag a column's left edge: its width;
    // right-click: which columns) -------------------------------------------------
    Rectangle {
        id: head
        width: parent.width; height: 30
        color: Theme.bg
        Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.surface1 }
        MouseArea {
            anchors.fill: parent
            acceptedButtons: Qt.RightButton
            enabled: !root.located
            onClicked: m => colMenu.openAt(m.x, m.y)
        }
        Menu {
            id: colMenu
            items: [
                { label: "Size", checked: Session.columns.includes("size"), run: () => Session.toggleColumn("size") },
                { label: "Modified", checked: Session.columns.includes("modified"), run: () => Session.toggleColumn("modified") },
                { label: "Created", checked: Session.columns.includes("created"), run: () => Session.toggleColumn("created") },
                { label: "Kind", checked: Session.columns.includes("kind"), run: () => Session.toggleColumn("kind") },
                { label: "Tags", checked: Session.columns.includes("tags"), run: () => Session.toggleColumn("tags") },
            ]
        }
        Row {
            anchors { fill: parent; leftMargin: 16; rightMargin: 16 }
            Repeater {
                model: [{ key: "name", label: root.renamingNow ? "Name  →  new name" : "Name", w: root.nameW, sort: "name" }].concat(root.cols)
                delegate: Item {
                    id: heading
                    required property var modelData
                    required property int index
                    readonly property string sort: root.pane.searching ? "" : modelData.sort
                    width: modelData.w; height: head.height
                    Row {
                        anchors.verticalCenter: parent.verticalCenter
                        x: heading.index > 0 ? 8 : 0
                        spacing: 4
                        Text {
                            text: modelData.label
                            color: heading.sort !== "" && root.pane.sortBy === heading.sort ? Theme.text : Theme.muted
                            font { family: Theme.font; pointSize: Theme.pt(11.5); weight: Font.Medium }
                        }
                        Icon {
                            visible: heading.sort !== "" && root.pane.sortBy === heading.sort
                            anchors.verticalCenter: parent.verticalCenter
                            name: root.pane.descending ? "chevDown" : "chevUp"; size: 11; color: Theme.muted
                        }
                    }
                    MouseArea {
                        anchors.fill: parent
                        cursorShape: heading.sort !== "" ? Qt.PointingHandCursor : Qt.ArrowCursor
                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                        onClicked: m => {
                            if (m.button === Qt.RightButton) { if (!root.located) colMenu.openAt(heading.x + m.x + 16, m.y); return }
                            if (heading.sort === "") return;
                            if (root.pane.sortBy === heading.sort) root.pane.setView({ descending: !root.pane.descending });
                            else root.pane.setView({ sortBy: heading.sort, descending: !["name", "kind"].includes(heading.sort) });
                        }
                    }
                    // Its left edge: drag to make it wider or narrower.
                    MouseArea {
                        visible: heading.index > 0
                        x: -5; width: 10; height: parent.height
                        cursorShape: Qt.SplitHCursor
                        property real startX: 0
                        property real startW: 0
                        onPressed: m => { startX = mapToItem(head, m.x, 0).x; startW = heading.modelData.w }
                        onPositionChanged: m => {
                            if (!pressed) return;
                            const w = Math.max(60, Math.min(600, startW - (mapToItem(head, m.x, 0).x - startX)));
                            Session.setWidth(heading.modelData.key, w, false);
                        }
                        onReleased: Session.setWidth(heading.modelData.key, heading.modelData.w, true)
                    }
                }
            }
        }
    }

    // ---- rows ----------------------------------------------------------------------
    ListView {
        id: list
        // Something new being named: it's first, so show the top.
        Connections { target: root.pane; function onNamingChanged() { if (root.pane.naming && root.pane.naming.what !== "rename") list.positionViewAtBeginning() } }
        anchors { left: parent.left; right: parent.right; top: head.bottom; bottom: parent.bottom }
        model: root.pane.shownEntries
        currentIndex: root.pane.current
        clip: true
        focus: true
        boundsBehavior: Flickable.StopAtBounds
        topMargin: 4; bottomMargin: 8
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
        highlightFollowsCurrentItem: false
        keyNavigationEnabled: true
        onCurrentIndexChanged: if (currentIndex !== root.pane.current) root.pane.current = currentIndex
        // Grouped: a heading before each group (click to fold it).
        section.property: root.pane.grouped ? "group" : ""
        section.delegate: GroupHeading {
            required property string section
            width: list.width - 16; x: 8
            height: 40
            label: section
            count: root.pane.shownEntries.filter(e => e.group === section).length
            folded: !!root.pane.folded[section]
            onToggled: root.pane.fold(section)
        }
        Keys.onReturnPressed: root.pane.activate(root.pane.shownEntries[root.pane.current])
        Keys.onEnterPressed: root.pane.activate(root.pane.shownEntries[root.pane.current])
        Keys.onPressed: e => {
            if (e.key === Qt.Key_Up || e.key === Qt.Key_Down)
                Qt.callLater(() => root.pane.clickAt(list.currentIndex, false, (e.modifiers & Qt.ShiftModifier) !== 0));
        }
        MouseArea {
            anchors.fill: parent
            z: -1
            acceptedButtons: Qt.LeftButton | Qt.RightButton
            onClicked: m => {
                root.pane.commitNaming();
                root.pane.selected = {};
                list.forceActiveFocus();
                if (m.button === Qt.RightButton) { const p = mapToItem(root.pane, m.x, m.y); root.pane.menuAt(null, p.x, p.y) }
            }
        }

        delegate: Rectangle {
            id: row
            required property var modelData
            required property int index
            readonly property bool on: !!root.pane.selected[modelData.path]
            readonly property bool hiddenByFold: root.pane.grouped && !!root.pane.folded[modelData.group]
            // (For screen readers and the UI tests: the file, whether it's
            // selected, and, while renaming, its new name.)
            Accessible.role: Accessible.ListItem
            Accessible.name: row.modelData.name ?? ""
            Accessible.description: row.newName ? "→ " + row.newName.to + (row.newName.problem ? " (" + row.newName.problem + ")" : "") : ""
            Accessible.selectable: true
            Accessible.selected: row.on

            x: 8; width: list.width - 16; height: hiddenByFold ? 0 : 32 * Session.zoom
            visible: !hiddenByFold
            readonly property string cloudState: root.pane.cloudOf(modelData)
            // Renaming in the Sheet: its new name ({to, problem}), if any.
            readonly property var newName: root.pane.renaming[modelData.path] ?? null
            // Selected next to selected: one block, not a box each.
            readonly property bool joinUp: on && index > 0 && !!root.pane.selected[root.pane.shownEntries[index - 1]?.path]
            readonly property bool joinDown: on && !!root.pane.selected[root.pane.shownEntries[index + 1]?.path]
            topLeftRadius: joinUp ? 0 : 8; topRightRadius: joinUp ? 0 : 8
            bottomLeftRadius: joinDown ? 0 : 8; bottomRightRadius: joinDown ? 0 : 8
            color: on || drop.over ? Theme.tint(0.22) : rowArea.containsMouse ? Theme.surface1 : "transparent"
            border { width: drop.over || (index === list.currentIndex && list.activeFocus) ? 1 : 0; color: Theme.tint(0.6) }
            Row {
                anchors { fill: parent; leftMargin: 8; rightMargin: 8 }
                Item {
                    width: root.nameW; height: parent.height
                    Image {
                        visible: !row.modelData.look
                        id: ic
                        anchors.verticalCenter: parent.verticalCenter
                        width: 20 * Session.zoom; height: 20 * Session.zoom
                        sourceSize: Qt.size(40, 40)
                        source: row.modelData.icon ? F.fileUrl(row.modelData.icon) : ""
                        asynchronous: true
                        opacity: row.modelData.hidden ? 0.55 : 1
                    }
                    // A folder with its own colour or icon.
                    FolderIcon {
                        visible: !!row.modelData.look
                        anchors.fill: ic
                        size: ic.width
                        look: row.modelData.look ?? ({})
                    }
                    CompareMark { look: root.pane.compareOf(row.modelData); anchors { right: ic.right; rightMargin: -3; bottom: ic.bottom; bottomMargin: -2 } }
                    // Being named (new, or renamed): typed right here.
                    Loader {
                        active: root.pane.isNaming(row.modelData)
                        anchors { left: ic.right; leftMargin: 6; right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
                        sourceComponent: NameEditor { pane: root.pane; dir: !!row.modelData.dir; fontSize: 12.5 * Math.sqrt(Session.zoom) }
                    }
                    Text {
                        id: nameText
                        visible: !root.pane.isNaming(row.modelData)
                        anchors { left: ic.right; leftMargin: 10; right: parent.right; rightMargin: row.modelData.found ? parent.width * 0.55 : row.newName ? parent.width * 0.5 : 8 + (row.cloudState ? 22 : 0); verticalCenter: parent.verticalCenter }
                        text: row.modelData.name + (row.modelData.link ? "  →" : "")
                        elide: Text.ElideMiddle
                        color: row.modelData.hidden || row.newName ? Theme.muted : Theme.text
                        font { family: Theme.font; pointSize: Theme.pt(12.5 * Math.sqrt(Session.zoom)) }
                    }
                    // Renaming: → the new name (red, with why, when it's left alone).
                    Row {
                        visible: !!row.newName
                        x: parent.width * 0.5; width: parent.width * 0.5 - 8
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 8
                        Icon { anchors.verticalCenter: parent.verticalCenter; name: "arrowRight"; size: 12; color: Theme.faint }
                        Text {
                            id: toText
                            anchors.verticalCenter: parent.verticalCenter
                            width: Math.min(implicitWidth, parent.width - 20 - (warnIcon.visible ? 20 : 0))
                            elide: Text.ElideMiddle
                            readonly property bool same: row.newName?.to === row.modelData.name
                            text: same && !row.newName?.problem ? "(not changing)" : row.newName?.to ?? ""
                            color: row.newName?.problem ? Theme.rose : same ? Theme.faint : Theme.text
                            font { family: Theme.font; pointSize: Theme.pt(12.5 * Math.sqrt(Session.zoom)); weight: same ? Font.Normal : Font.Medium }
                        }
                        Icon {
                            id: warnIcon
                            visible: !!row.newName?.problem
                            anchors.verticalCenter: parent.verticalCenter
                            name: "alert"; size: 12; color: Theme.rose
                            MouseArea { id: warnMouse; anchors { fill: parent; margins: -4 } hoverEnabled: true }
                            ToolTip.visible: warnMouse.containsMouse
                            ToolTip.text: row.newName?.problem ?? ""
                        }
                    }
                    // Searching inside: where it was found.
                    Text {
                        visible: !!row.modelData.found
                        anchors { left: parent.left; leftMargin: ic.width + 10 + Math.min(nameText.implicitWidth, nameText.width) + 14; right: parent.right; rightMargin: 8; verticalCenter: parent.verticalCenter }
                        text: row.modelData.found ?? ""
                        textFormat: Text.PlainText
                        elide: Text.ElideRight
                        color: Theme.faint
                        font { family: Theme.mono; pointSize: Theme.pt(10.5) }
                    }
                    CloudBadge {
                        state: row.cloudState
                        anchors { left: parent.left; leftMargin: ic.width + 10 + Math.min(nameText.implicitWidth, nameText.width) + 8; verticalCenter: parent.verticalCenter }
                    }
                }
                Repeater {
                    model: root.cols
                    delegate: Text {
                        required property var modelData
                        width: modelData.w
                        anchors.verticalCenter: parent.verticalCenter
                        leftPadding: 8
                        // A name left alone: why, in place of the date.
                        readonly property bool why: !!row.newName?.problem && modelData.key === "modified"
                        text: why ? "left as it is" : modelData.value(row.modelData)
                        textFormat: modelData.rich && !why ? Text.StyledText : Text.PlainText
                        elide: modelData.key === "where" ? Text.ElideMiddle : Text.ElideRight
                        color: why ? Theme.rose : Theme.muted
                        font { family: Theme.font; pointSize: Theme.pt(12) }
                    }
                }
            }
            DragSource { id: carry; paths: root.pane.selection.map(e => e.path); icon: row.modelData.icon ?? ""; }
            FolderDrop { id: drop; anchors.fill: parent; enabled: row.modelData.dir; dest: row.modelData.path; onFilesDropped: (p, d) => root.pane.dropped(p, d) }
            MouseArea {
                id: rowArea
                anchors.fill: parent
                hoverEnabled: true
                acceptedButtons: Qt.LeftButton | Qt.RightButton
                onPositionChanged: m => { if (rowArea.pressedButtons & Qt.LeftButton) carry.moved(m.x, m.y) }
                onPressed: m => {
                    if (m.button === Qt.LeftButton) carry.press(m.x, m.y);
                    if (m.button === Qt.LeftButton && !row.on && !(m.modifiers & (Qt.ControlModifier | Qt.ShiftModifier)))
                        root.pane.clickAt(row.index, false, false);
                }
                onClicked: m => {
                    list.forceActiveFocus();
                    if (m.button === Qt.RightButton) {
                        if (!row.on) root.pane.clickAt(row.index, false, false);
                        const p = mapToItem(root.pane, m.x, m.y);
                        root.pane.menuAt(row.modelData, p.x, p.y);
                        return;
                    }
                    root.pane.pick(row.index, m.modifiers & Qt.ControlModifier, m.modifiers & Qt.ShiftModifier);
                }
                onDoubleClicked: root.pane.activate(row.modelData)
            }
        }
    }
}
