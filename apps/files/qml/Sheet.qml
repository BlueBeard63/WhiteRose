// The Sheet: beside the folder (Alt+P), what's selected, looked at and
// changed in place: no windows over the files. Tabs along the top:
//   Info     the file (preview, name changed by typing on it, kind, size,
//            opens with, where, when, tags, comment, checksums, who may
//            do what); several: how many and how big; none: the folder.
//   Rename   rules for every selected name, the new names shown in the
//            list as you type (RenameSheet.qml).
//   Share    copy where it is, or compress it (ShareSheet.qml).
//   Compare, Sync  two folders compared, then synced (CompareSheet.qml):
//            their own pair of tabs while you do it.
//   Server, Account  connecting to a network share or adding a cloud
//            account (ConnectSheet.qml, CloudSheet.qml): their own pair of
//            tabs while you do it.
//   Scripts  the folder's project scripts (Cargo, npm, Go, Make…), run in
//            your terminal; somewhere untrusted (Downloads…) asks first.
// (Properties, the tag and comment boxes all live here now.)
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F

Rectangle {
    id: root
    property var pane: null
    property string tab: "info"
    signal runScript(string dir, string command)
    signal tagsEdited()
    signal closeIt()
    signal said(string notice)
    signal renamed(var paths)
    signal renameFailed(string message)
    signal serversChanged()
    signal openPath(string path)
    signal cloudAdded(string name)
    // Connecting to a server or a cloud account (from the sidebar or ☰).
    readonly property bool networking: root.tab === "server" || root.tab === "account"
    readonly property bool comparing: root.tab === "compare" || root.tab === "sync"
    // Its own tabs (connecting, comparing): the usual ones are put away.
    readonly property bool busyMode: root.networking || root.comparing
    property var panes: []
    signal made(int job)
    signal synced(int job)
    function showCompare(left, right) { comparer.start(left, right); root.tab = "compare" }
    function showShare() { root.tab = "share" }
    function showServer(server) { connector.show(server); root.tab = "server" }
    function showCloud() { clouder.show(); root.tab = "account" }
    function connectError(error, question) { connector.error = error; connector.question = question }

    readonly property var sel: root.pane?.selection ?? []
    readonly property var entry: root.sel.length === 1 ? root.sel[0] : null
    // What the Info tab is about: what's selected, else the folder itself.
    readonly property var targets: root.sel.length > 0 ? root.sel.map(e => e.path)
        : root.pane && root.pane.path.startsWith("/") && !root.pane.special ? [root.pane.path] : []
    readonly property bool one: root.targets.length === 1
    readonly property bool typing: nameField.editing || commentField.editing || tagField.editing || pasted.editing || renamer.typing || connector.typing || clouder.typing || sharer.typing

    // ---- what's known about the targets ------------------------------------------------
    property var info: ({})
    property var count: ({ bytes: 0, files: 0, folders: 0, done: true })
    property int measuring: -1
    property var apps: []
    readonly property var defaultApp: root.apps.find(a => a.default) ?? null
    property string comment: ""
    property string error: ""
    property var sums: null
    property real sumDone: -1
    property bool adding: false
    readonly property bool canRename: root.one && root.info.ok === true && !root.info.origin
    readonly property bool canChange: root.one && root.info.mine === true && !root.info.link && !root.info.origin
    readonly property var tags: root.entry?.tags ?? []

    property string lookedAt: ""
    function look() {
        const key = JSON.stringify(root.targets);
        if (key === root.lookedAt || !root.visible) return;
        // Whatever was being typed about the last one is kept.
        if (root.one && root.lookedAt !== "") { root.saveName(); root.saveComment() }
        root.lookedAt = key;
        Ops.stopMeasuring(root.measuring); root.measuring = -1; Ops.hash("");
        root.error = ""; root.sums = null; root.sumDone = -1; root.adding = false;
        pasted.text = "";
        root.info = root.one ? JSON.parse(Fs.info(root.targets[0])) : {};
        root.comment = root.one ? Fs.comment(root.targets[0]) : "";
        commentField.text = root.comment;
        nameField.text = root.info.name ?? "";
        root.apps = root.one && root.info.ok && !root.info.dir ? JSON.parse(Fs.appsFor(root.info.mime ?? "")) : [];
        // Files' sizes are known; folders are counted in the background.
        const needs = root.targets.length > 0 && !(root.one && root.info.ok && !root.info.dir);
        root.count = { bytes: 0, files: 0, folders: 0, done: !needs };
        if (needs) root.measuring = Ops.measure(JSON.stringify(root.targets));
    }
    onTargetsChanged: Qt.callLater(root.look)
    onVisibleChanged: if (visible) { root.lookedAt = ""; Qt.callLater(root.look) } else { Ops.stopMeasuring(root.measuring); root.measuring = -1 }
    function refresh() { root.lookedAt = ""; root.look() }

    function saveName() {
        const n = nameField.text.trim();
        if (!root.canRename) return;
        if (n === "" || n === root.info.name) { nameField.text = root.info.name ?? ""; return }
        const r = JSON.parse(Ops.rename(root.info.path, n));
        if (!r.ok) { root.error = r.error; nameField.text = root.info.name ?? ""; return }
        root.error = "";
        root.info = JSON.parse(Fs.info(r.path));
        root.lookedAt = JSON.stringify([r.path]);
        root.pane?.load();
    }
    function saveComment() {
        if (!root.canRename || commentField.text === root.comment) return;
        const err = Fs.setComment(root.info.path, commentField.text);
        root.error = err;
        if (err === "") root.comment = commentField.text.trim();
    }
    function setBit(who, what, on) {
        const err = Fs.setMode(root.info.path, F.withBit(root.info.mode, who, what, on));
        root.error = err;
        if (err === "") root.info = JSON.parse(Fs.info(root.info.path));
    }
    function tag(name, on) {
        const r = JSON.parse(Fs.setTag(JSON.stringify(root.targets), name, on));
        if (r.failed.length > 0) { root.error = r.failed[0][1]; return }
        if (on && !Session.tagDefs.some(t => t.name.toLowerCase() === name.toLowerCase())) Session.defineTag(name, Session.freshColour());
        root.tagsEdited();
    }
    function addTag() {
        const c = JSON.parse(Fs.checkTag(tagField.text));
        if (!c.ok) { root.error = c.error; return }
        root.error = "";
        root.tag(c.name, true);
        tagField.text = "";
        root.adding = false;
    }

    Connections {
        target: Ops
        function onMeasured(id, result) { if (id === root.measuring) root.count = JSON.parse(result) }
        function onHashed(path, result) {
            if (!root.one || path !== root.targets[0]) return;
            const r = JSON.parse(result);
            if (!r.done) { root.sumDone = r.total > 0 ? r.bytes / r.total : 0; return }
            root.sumDone = -1;
            if (r.error) root.error = r.error; else root.sums = r;
        }
    }

    // ---- the folder's projects (Scripts) -------------------------------------------------
    property var projects: ({ projects: [], why: "" })
    property bool trusted: false
    function lookAtProjects() {
        root.trusted = false;
        root.projects = root.pane && root.pane.path.startsWith("/") && !root.pane.special ? JSON.parse(Fs.projects(root.pane.path)) : { projects: [], why: "" };
        if (root.tab === "scripts" && root.projects.projects.length === 0) root.tab = "info";
    }
    Connections { target: root.pane; function onPathChanged() { root.lookAtProjects() } }
    onPaneChanged: lookAtProjects()

    color: Theme.bg
    Rectangle { width: 1; height: parent.height; color: Theme.surface1 }

    // ---- pieces ----------------------------------------------------------------------------
    // A label and its value, side by side.
    component Fact: Row {
        property string label
        property string value
        visible: value !== ""
        width: parent.width
        spacing: 10
        Text { width: 84; text: parent.label; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
        Text {
            width: parent.width - 94
            text: parent.value
            wrapMode: Text.WrapAnywhere
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(11.5) }
        }
    }
    component Heading: Text {
        color: Theme.faint
        font { family: Theme.font; pointSize: Theme.pt(10); weight: Font.Medium; capitalization: Font.AllUppercase; letterSpacing: 0.6 }
    }
    // A tab: its name, underlined when it's the one shown.
    component Tab: Item {
        id: t
        property string name
        property string label
        readonly property bool on: root.tab === t.name
        width: tabText.implicitWidth; height: 34
        Text {
            id: tabText
            anchors.verticalCenter: parent.verticalCenter
            text: t.label
            color: t.on ? Theme.text : tabMouse.containsMouse ? Theme.text : Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(12); weight: t.on ? Font.Medium : Font.Normal }
        }
        Rectangle { visible: t.on; anchors.bottom: parent.bottom; width: parent.width; height: 2; radius: 1; color: Theme.rose }
        MouseArea { id: tabMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: root.tab = t.name }
    }

    // ---- tabs ------------------------------------------------------------------------------
    Item {
        id: head
        x: 16; width: parent.width - 32; height: 44
        Row {
            anchors.bottom: parent.bottom
            spacing: 18
            Tab { name: "server"; label: "Server"; visible: root.networking }
            Tab { name: "account"; label: "Account"; visible: root.networking; onVisibleChanged: if (visible && root.tab === "account" && clouder.info.providers.length === 0) clouder.show() }
            Tab { name: "compare"; label: "Compare"; visible: root.comparing }
            Tab { name: "sync"; label: "Sync"; visible: root.comparing }
            Tab { name: "info"; label: "Info"; visible: !root.busyMode }
            Tab { name: "rename"; label: "Rename"; visible: !root.busyMode && !(root.pane?.readOnly ?? true) && !(root.pane?.special ?? true) && (root.sel.length > 0 || root.tab === "rename") }
            Tab { name: "share"; label: "Share"; visible: !root.busyMode && (root.sel.length > 0 || root.tab === "share") }
            Tab { name: "scripts"; label: "Scripts"; visible: !root.busyMode && root.projects.projects.length > 0 }
        }
        IconButton {
            anchors { right: parent.right; verticalCenter: parent.verticalCenter; verticalCenterOffset: 3 }
            width: 26; height: 26; icon: "x"; iconSize: 12; fill: "transparent"
            onClicked: root.closeIt()
        }
        Rectangle { anchors.bottom: parent.bottom; width: parent.width; height: 1; color: Theme.surface1; z: -1 }
    }

    Flickable {
        anchors { left: parent.left; leftMargin: 1; right: parent.right; top: head.bottom; bottom: parent.bottom }
        contentHeight: (root.tab === "info" ? info.implicitHeight : root.tab === "rename" ? renamer.implicitHeight
            : root.tab === "server" ? connector.implicitHeight : root.tab === "account" ? clouder.implicitHeight
            : root.comparing ? comparer.implicitHeight : root.tab === "share" ? sharer.implicitHeight : scripts.implicitHeight) + 32
        clip: true
        boundsBehavior: Flickable.StopAtBounds

        // ======== Info ========
        Column {
            id: info
            // A column of the who-can table.
            readonly property real permW: Math.floor((width - 72) / 3)
            visible: root.tab === "info"
            x: 16; y: 14
            width: parent.width - 32
            spacing: 12

            Text {
                visible: root.targets.length === 0
                width: parent.width
                wrapMode: Text.Wrap
                text: "Nothing to show here."
                color: Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(12) }
            }

            // ---- the preview (a file selected) ----
            Rectangle {
                visible: root.entry !== null
                width: parent.width; height: Math.round(width * 0.62)
                radius: 12
                color: Theme.surface1
                clip: true
                Preview {
                    id: preview
                    anchors { fill: parent; margins: 6 }
                    entry: root.entry
                    visible: !root.entry?.look
                }
                FolderIcon {
                    visible: !!root.entry?.look
                    anchors.centerIn: parent
                    size: Math.min(parent.width, parent.height) * 0.5
                    look: root.entry?.look ?? ({})
                }
            }

            // ---- its name: type on it to rename ----
            Column {
                visible: root.targets.length > 0
                width: parent.width
                spacing: 2
                Item {
                    width: parent.width; height: 30
                    Text {
                        visible: !root.canRename
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width
                        text: root.one ? (root.info.name ?? "") : root.targets.length + " items"
                        elide: Text.ElideMiddle
                        color: Theme.text
                        font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
                    }
                    // Looks like the name until it's clicked.
                    TextInput {
                        id: nameField
                        visible: root.canRename
                        readonly property bool editing: activeFocus
                        anchors.verticalCenter: parent.verticalCenter
                        width: parent.width
                        clip: true
                        selectByMouse: true
                        color: Theme.text
                        selectionColor: Theme.tint(0.4)
                        font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
                        onAccepted: { root.saveName(); focus = false }
                        onEditingFinished: root.saveName()
                        Keys.onEscapePressed: { text = root.info.name ?? ""; focus = false }
                        Rectangle { anchors { left: parent.left; right: parent.right; top: parent.bottom; topMargin: 2 } height: 1; color: nameField.activeFocus ? Theme.rose : nameHover.containsMouse ? Theme.surface2 : "transparent" }
                        MouseArea { id: nameHover; anchors.fill: parent; hoverEnabled: true; acceptedButtons: Qt.NoButton; cursorShape: Qt.IBeamCursor }
                    }
                }
                Text {
                    visible: root.canRename
                    text: nameField.activeFocus ? "Enter to rename · Esc to leave it" : "Click the name to rename it here"
                    color: Theme.faint
                    font { family: Theme.font; pointSize: Theme.pt(10) }
                }
            }

            // ---- what it is, how big, where, when ----
            Column {
                visible: root.targets.length > 0
                width: parent.width
                spacing: 7
                Fact { label: "Kind"; value: !root.one ? "" : root.info.link ? "Link" : root.info.dir ? "Folder" : F.kind(root.info.mime ?? "") }
                Fact {
                    label: preview.kind === "image" ? "Dimensions" : preview.kind === "pdf" ? "Pages" : preview.kind === "video" || preview.kind === "audio" ? "Length" : ""
                    value: root.entry && label !== "" ? preview.facts : ""
                }
                Fact {
                    label: "Size"
                    value: {
                        const c = root.count;
                        if (root.one && root.info.ok && !root.info.dir) return F.exactSize(root.info.size ?? 0);
                        const parts = [];
                        if (c.files > 0 || c.done) parts.push(c.files + (c.files === 1 ? " file" : " files"));
                        if (c.folders > 0) parts.push(c.folders + (c.folders === 1 ? " folder" : " folders"));
                        return parts.join(", ") + (c.bytes > 0 || c.done ? ", " + F.size(c.bytes) : "") + (c.done ? "" : "…");
                    }
                }
                // The default for every file of this kind (mimeapps.list).
                Row {
                    visible: root.one && !root.info.dir && root.apps.length > 0 && !root.info.origin
                    width: parent.width
                    spacing: 10
                    Text { width: 84; anchors.verticalCenter: parent.verticalCenter; text: "Opens with"; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
                    Choice {
                        value: root.defaultApp?.id ?? ""
                        options: root.apps.map(a => ({ label: a.name, value: a.id }))
                        onPicked: v => { Fs.setDefaultApp(root.info.mime, v); root.apps = JSON.parse(Fs.appsFor(root.info.mime)) }
                    }
                }
                Fact { label: "Where"; value: root.one && !root.info.origin ? String(root.info.location ?? "").replace(Fs.home(), "~") : "" }
                Fact { label: "Link to"; value: root.one && root.info.target ? root.info.target : "" }
                Fact { label: "From"; value: root.one && root.info.origin ? String(root.info.origin).replace(Fs.home(), "~") : "" }
                Fact { label: "Found"; value: root.entry?.found ?? "" }
                Fact { label: "Deleted"; value: root.one ? F.fullDate(root.info.deleted ?? 0) : "" }
                Fact { label: "Modified"; value: root.one ? F.fullDate(root.info.modified) : "" }
                Fact { label: "Opened"; value: root.one ? F.fullDate(root.info.accessed) : "" }
                Fact { label: "Created"; value: root.one ? F.fullDate(root.info.created) : "" }
            }

            // ---- tags: a dot and a name; click one for every file with it ----
            Flow {
                visible: root.entry !== null && !root.pane?.inTrash
                width: parent.width
                spacing: 14
                Repeater {
                    model: root.tags
                    delegate: Row {
                        id: tagRow
                        required property string modelData
                        spacing: 6
                        height: 26
                        Rectangle { width: 8; height: 8; radius: 4; color: Session.tagColour(tagRow.modelData); anchors.verticalCenter: parent.verticalCenter }
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: tagRow.modelData
                            color: tagMouse.containsMouse ? Theme.rose : Theme.text
                            font { family: Theme.font; pointSize: Theme.pt(11.5) }
                            MouseArea { id: tagMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: root.pane.go("tag:" + tagRow.modelData) }
                        }
                        // Take it off (shown on hover).
                        Icon {
                            anchors.verticalCenter: parent.verticalCenter
                            name: "x"; size: 10
                            color: offMouse.containsMouse ? Theme.text : Theme.faint
                            opacity: tagMouse.containsMouse || offMouse.containsMouse ? 1 : 0
                            MouseArea { id: offMouse; anchors { fill: parent; margins: -4 } hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: root.tag(tagRow.modelData, false) }
                        }
                    }
                }
                // Add one: typed here (a known one or a new one).
                Text {
                    visible: !root.adding
                    height: 26
                    verticalAlignment: Text.AlignVCenter
                    text: "Add tag"
                    color: addTagMouse.containsMouse ? Theme.text : Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(11.5) }
                    MouseArea { id: addTagMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: { root.adding = true; tagField.focusField() } }
                }
                Field {
                    id: tagField
                    visible: root.adding
                    implicitWidth: 150; implicitHeight: 28
                    placeholder: Session.tagDefs.filter(t => !root.tags.includes(t.name)).map(t => t.name).slice(0, 2).join(", ") || "A tag"
                    onAccepted: root.addTag()
                    onEditingChanged: if (!editing && text.trim() === "") root.adding = false
                    Keys.onEscapePressed: { text = ""; root.adding = false }
                }
            }

            // ---- a note kept on the file (other file managers show it too) ----
            Field {
                id: commentField
                visible: root.canRename
                width: parent.width; height: 34
                placeholder: "Add a comment…"
                onAccepted: root.saveComment()
                onEditingChanged: if (!editing) root.saveComment()
            }

            // ---- checksums (one file; worked out when asked: they read it all) ----
            Column {
                visible: root.one && root.info.ok === true && !root.info.dir && !root.info.link
                width: parent.width
                spacing: 7
                Item { width: 1; height: 2 }
                Row {
                    width: parent.width
                    spacing: 10
                    Heading { width: 84; anchors.verticalCenter: parent.verticalCenter; text: "Checksums" }
                    Text {
                        visible: root.sums === null
                        anchors.verticalCenter: parent.verticalCenter
                        text: root.sumDone >= 0 ? "Reading… " + Math.round(root.sumDone * 100) + "%" : "Work them out"
                        color: sumMouse.containsMouse && root.sumDone < 0 ? Theme.text : Theme.muted
                        font { family: Theme.font; pointSize: Theme.pt(11.5) }
                        MouseArea { id: sumMouse; anchors.fill: parent; enabled: root.sumDone < 0; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: { root.sumDone = 0; Ops.hash(root.targets[0]) } }
                    }
                }
                Repeater {
                    model: root.sums ? [["MD5", root.sums.md5], ["SHA-1", root.sums.sha1], ["SHA-256", root.sums.sha256]] : []
                    delegate: Row {
                        required property var modelData
                        width: parent.width
                        spacing: 10
                        Text { width: 84; anchors.verticalCenter: parent.verticalCenter; text: modelData[0]; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
                        Text {
                            width: parent.width - 94 - 30
                            anchors.verticalCenter: parent.verticalCenter
                            text: modelData[1]
                            wrapMode: Text.WrapAnywhere
                            color: F.sumMatch(root.sums, pasted.text) === modelData[0] ? Theme.rose : Theme.text
                            font { family: Theme.mono; pointSize: Theme.pt(9.5) }
                        }
                        IconButton {
                            width: 24; height: 24; icon: "copy"; iconSize: 11; fill: "transparent"
                            anchors.verticalCenter: parent.verticalCenter
                            onClicked: Ops.setClipboardText(modelData[1])
                        }
                    }
                }
                // Paste the sum a download page gives: does it match?
                Field {
                    id: pasted
                    visible: root.sums !== null
                    width: parent.width; height: 32
                    mono: true
                    placeholder: "Paste a checksum to compare"
                    error: text.trim() !== "" && F.sumMatch(root.sums, text) === ""
                }
                Text {
                    visible: root.sums !== null && pasted.text.trim() !== ""
                    text: F.sumMatch(root.sums, pasted.text) !== "" ? "✓ Matches its " + F.sumMatch(root.sums, pasted.text) : "✗ Doesn't match any of these"
                    color: F.sumMatch(root.sums, pasted.text) !== "" ? Theme.rose : Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(11) }
                }
            }

            // ---- who may do what (one thing; changed straight away) ----
            Column {
                visible: root.one && root.info.ok === true && !root.info.origin
                width: parent.width
                spacing: 6
                Item { width: 1; height: 2 }
                Heading { text: "Who can" }
                Fact { label: "Owner"; value: (root.info.owner ?? "") + (root.info.group ? " · group " + root.info.group : "") }
                Row {
                    Item { width: 72; height: 1 }
                    Repeater {
                        model: ["Read", "Change", root.info.dir ? "Open" : "Run"]
                        delegate: Text {
                            required property string modelData
                            width: info.permW
                            text: modelData
                            color: Theme.faint
                            font { family: Theme.font; pointSize: Theme.pt(10) }
                        }
                    }
                }
                Repeater {
                    model: ["You", "Group", "Everyone"]
                    delegate: Row {
                        required property string modelData
                        required property int index
                        readonly property int who: index
                        height: 28
                        Text {
                            width: 72
                            anchors.verticalCenter: parent.verticalCenter
                            text: index === 0 && !root.info.mine ? "Owner" : modelData
                            color: Theme.muted
                            font { family: Theme.font; pointSize: Theme.pt(11.5) }
                        }
                        Repeater {
                            model: 3
                            delegate: Item {
                                required property int index
                                width: info.permW; height: 28
                                Toggle {
                                    anchors { left: parent.left; verticalCenter: parent.verticalCenter }
                                    checked: F.hasBit(root.info.mode ?? 0, parent.parent.who, index)
                                    enabled: root.canChange
                                    onToggled: on => root.setBit(parent.parent.who, index, on)
                                }
                            }
                        }
                    }
                }
            }

            Text {
                visible: root.error !== ""
                width: parent.width
                text: root.error
                wrapMode: Text.Wrap
                color: Theme.rose
                font { family: Theme.font; pointSize: Theme.pt(11) }
            }
            Button {
                visible: root.entry !== null
                anchors.verticalCenter: undefined
                text: "Open"; kind: "primary"
                onClicked: root.pane.activate(root.entry)
            }
        }

        // ======== Rename ========
        RenameSheet {
            id: renamer
            visible: root.tab === "rename"
            active: root.visible && root.tab === "rename"
            x: 16; y: 14
            width: parent.width - 32
            pane: root.pane
            onRenamed: paths => { root.tab = "info"; root.renamed(paths) }
            onFailed: message => root.renameFailed(message)
            onCancelled: root.tab = "info"
        }

        // ======== Server / Account ========
        ConnectSheet {
            id: connector
            visible: root.tab === "server"
            x: 16; y: 14
            width: parent.width - 32
            onChanged: root.serversChanged()
            onOpened_: path => root.openPath(path)
            onClosed: root.tab = "info"
        }
        CloudSheet {
            id: clouder
            visible: root.tab === "account"
            x: 16; y: 14
            width: parent.width - 32
            onAdded: name => root.cloudAdded(name)
            onClosed: root.tab = "info"
        }

        // ======== Share ========
        ShareSheet {
            id: sharer
            visible: root.tab === "share"
            x: 16; y: 14
            width: parent.width - 32
            pane: root.pane
            onMade: id => { root.made(id); root.tab = "info" }
            onSaid: text => root.said(text)
        }

        // ======== Compare / Sync ========
        CompareSheet {
            id: comparer
            visible: root.comparing
            part: root.tab === "sync" ? "sync" : "compare"
            x: 16; y: 14
            width: parent.width - 32
            panes: root.panes
            onSynced: id => root.synced(id)
            onClosed: root.tab = "info"
        }

        // ======== Scripts ========
        Column {
            id: scripts
            visible: root.tab === "scripts"
            x: 16; y: 14
            width: parent.width - 32
            spacing: 10
            // Somewhere untrusted: asked first.
            Rectangle {
                visible: root.projects.why !== "" && !root.trusted
                width: parent.width
                height: warn.implicitHeight + trust.height + 26
                radius: 10
                color: Theme.surface1
                border { width: 1; color: Theme.rose }
                Text {
                    id: warn
                    x: 10; y: 10; width: parent.width - 20
                    wrapMode: Text.Wrap
                    text: root.projects.why + " Its scripts run whatever the project says, so only run them if you trust where it came from."
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(11) }
                }
                Button { id: trust; x: 10; y: warn.y + warn.height + 6; anchors.verticalCenter: undefined; text: "I trust this project"; kind: "danger"; onClicked: root.trusted = true }
            }
            Repeater {
                model: root.projects.projects
                delegate: Column {
                    id: proj
                    required property var modelData
                    width: parent.width
                    spacing: 0
                    Heading { height: 22; text: proj.modelData.kind + "  ·  " + proj.modelData.file }
                    Repeater {
                        model: proj.modelData.scripts
                        delegate: Rectangle {
                            id: sc
                            required property var modelData
                            readonly property bool can: root.projects.why === "" || root.trusted
                            width: parent.width; height: 32; radius: 8
                            color: scArea.containsMouse && sc.can ? Theme.surface1 : "transparent"
                            Icon { x: 8; anchors.verticalCenter: parent.verticalCenter; name: "play"; size: 11; color: sc.can ? Theme.muted : Theme.faint }
                            Text {
                                x: 28; anchors.verticalCenter: parent.verticalCenter
                                width: parent.width - 34
                                elide: Text.ElideRight
                                text: sc.modelData.name
                                color: sc.can ? Theme.text : Theme.faint
                                font { family: Theme.font; pointSize: Theme.pt(12) }
                            }
                            MouseArea {
                                id: scArea
                                anchors.fill: parent
                                enabled: sc.can
                                hoverEnabled: true
                                cursorShape: Qt.PointingHandCursor
                                onClicked: root.runScript(root.pane.path, sc.modelData.command)
                                ToolTip.visible: containsMouse
                                ToolTip.delay: 500
                                ToolTip.text: "Runs “" + sc.modelData.command + "” in your terminal"
                            }
                        }
                    }
                }
            }
        }
    }
}
