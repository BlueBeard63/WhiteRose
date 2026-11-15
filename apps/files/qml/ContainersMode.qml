// Containers ("containers:"): every Docker or Podman container, grouped by
// Compose project (containers.rs). Start, stop, restart, remove (asked);
// a Compose project up or down; its logs or a shell in your terminal; and
// its files, looked at inside it and copied out to the folder you choose.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F

Item {
    id: page
    required property var pane

    property string engine: ""
    property string error: ""
    property bool loading: true
    property var containers: []
    // Jobs in flight: {job: label}.
    property var busy: ({})
    // Looking inside one: {id, name} and where, with what's there.
    property var inside: null
    property string at: "/"
    property var entries: []
    property string insideError: ""

    function refresh() { Ops.askContainers() }
    Component.onCompleted: refresh()
    Timer { interval: 5000; repeat: true; running: page.visible && page.inside === null && Qt.application.state === Qt.ApplicationActive; onTriggered: page.refresh() }
    Connections {
        target: Ops
        function onContainersFound(result) {
            const r = JSON.parse(result);
            page.loading = false;
            page.engine = r.engine;
            page.error = r.error ?? "";
            page.containers = r.containers;
        }
        function onContainerDone(job, result) {
            if (!(job in page.busy)) return;
            const r = JSON.parse(result);
            const b = Object.assign({}, page.busy);
            const what = b[job];
            delete b[job];
            page.busy = b;
            if (!r.ok) page.Window.window.notice = r.error;
            else if (r.path) page.Window.window.info = "Copied out to " + String(r.path).replace(Fs.home(), "~");
            else page.Window.window.info = what;
            page.refresh();
        }
    }
    function act(req, said) {
        const job = Ops.containerDo(JSON.stringify(req));
        const b = Object.assign({}, page.busy);
        b[job] = said;
        page.busy = b;
    }
    function working() { return Object.keys(page.busy).length > 0 }
    function terminal(what, c) {
        const cmd = Ops.containerCommand(what, c.id);
        if (!cmd) return;
        const err = Fs.runInTerminal(Fs.home(), cmd, page.Window.window.choice("apps.terminal"));
        if (err) page.Window.window.notice = err;
    }
    // Grouped: [{project, workdir, items}] ("" project: on their own).
    readonly property var groups: {
        const g = [];
        for (const c of page.containers) {
            let x = g.find(y => y.project === c.project);
            if (!x) { x = { project: c.project, workdir: c.workdir, items: [] }; g.push(x) }
            x.items.push(c);
        }
        return g.sort((a, b) => (a.project === "") - (b.project === "") || a.project.localeCompare(b.project));
    }

    // ---- looking inside -------------------------------------------------------------
    function look(c, path) {
        page.inside = c;
        page.at = path;
        const r = JSON.parse(Ops.containerLs(c.id, path));
        page.insideError = r.error ?? "";
        page.entries = r.entries ?? [];
    }
    function join(dir, name) { return (dir.endsWith("/") ? dir : dir + "/") + name }
    function up() { const p = page.at.replace(/\/[^/]*\/?$/, ""); page.look(page.inside, p || "/") }
    FolderDialog {
        id: copyTo
        property string path: ""
        title: "Copy “" + copyTo.path.substring(copyTo.path.lastIndexOf("/") + 1) + "” out to…"
        currentFolder: F.fileUrl(Fs.home())
        onAccepted: page.act({ what: "copy", id: page.inside.id, path: copyTo.path, dest: decodeURIComponent(String(selectedFolder).replace(/^file:\/\//, "")) }, "Copied out")
    }

    // ---- removing asks (in a strip above the list) ------------------------------------------------------------------
    Strip {
        id: ask
        property var c: null
        anchors { left: parent.left; right: parent.right; top: head.bottom; topMargin: asking ? 8 : 0 }
        tone: "danger"
        icon: "trash"
        focusOn: removeButton
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: "Remove “" + (ask.c?.name ?? "") + "”?"
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(12); weight: Font.Medium }
        }
        Text {
            anchors.verticalCenter: parent.verticalCenter
            width: Math.max(60, ask.width - 520)
            elide: Text.ElideRight
            text: "Anything it keeps inside itself (not in a volume) goes with it. Its image stays."
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(11) }
        }
        buttons: [
            Button { text: "Keep it"; kind: "ghost"; onClicked: ask.close() },
            Button { id: removeButton; text: "Remove"; kind: "danger"; onClicked: { page.act({ what: "remove", id: ask.c.id }, "Removed " + ask.c.name); ask.close() } }
        ]
    }

    // ---- the list ---------------------------------------------------------------------------
    Column {
        id: head
        x: 24; y: 20
        width: page.width - 48
        spacing: 6
        Row {
            spacing: 10
            Text {
                text: page.inside ? page.inside.name : "Containers"
                color: Theme.text
                font { family: Theme.font; pointSize: Theme.pt(20); weight: Font.DemiBold }
            }
            Text {
                anchors.baseline: parent.children[0].baseline
                text: page.inside ? page.at : page.engine ? page.engine.charAt(0).toUpperCase() + page.engine.slice(1) : ""
                color: Theme.muted
                font { family: page.inside ? Theme.mono : Theme.font; pointSize: Theme.pt(12) }
            }
        }
        Row {
            visible: page.inside !== null
            spacing: 8
            Button { text: "Back to containers"; onClicked: page.inside = null }
            Button { text: "Up"; enabled: page.at !== "/"; onClicked: page.up() }
            Button { text: "Copy this folder out…"; onClicked: { copyTo.path = page.at; copyTo.open() } }
        }
    }

    Text {
        visible: !page.inside && (page.loading || page.error !== "" || page.engine === "" || page.containers.length === 0)
        anchors.centerIn: parent
        width: page.width - 80
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.Wrap
        text: page.loading ? "Looking…" : page.engine === "" ? "Neither Docker nor Podman is installed." : page.error !== "" ? page.error : "No containers."
        color: page.error ? Theme.rose : Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(12.5) }
    }

    ListView {
        id: list
        visible: page.inside === null
        anchors { left: parent.left; right: parent.right; top: ask.bottom; topMargin: 14; bottom: parent.bottom }
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
        model: page.groups
        spacing: 6
        delegate: Column {
            id: grp
            required property var modelData
            width: list.width
            // A Compose project: its name, folder, and the whole project's actions.
            Item {
                visible: grp.modelData.project !== ""
                width: parent.width; height: visible ? 40 : 0
                Text {
                    x: 24; anchors.verticalCenter: parent.verticalCenter
                    text: grp.modelData.project + "  ·  " + grp.modelData.items.filter(c => c.state === "running").length + " of " + grp.modelData.items.length + " running"
                    color: Theme.faint
                    font { family: Theme.font; pointSize: Theme.pt(10.5); weight: Font.Medium; capitalization: Font.AllUppercase; letterSpacing: 0.6 }
                }
                Row {
                    anchors { right: parent.right; rightMargin: 24; verticalCenter: parent.verticalCenter }
                    spacing: 6
                    visible: grp.modelData.workdir !== ""
                    Button { text: "Up"; enabled: !page.working(); onClicked: page.act({ what: "compose", action: "up", workdir: grp.modelData.workdir }, grp.modelData.project + " is up") }
                    Button { text: "Down"; enabled: !page.working(); onClicked: page.act({ what: "compose", action: "down", workdir: grp.modelData.workdir }, grp.modelData.project + " is down") }
                    Button { text: "Folder"; kind: "ghost"; onClicked: page.pane.go(grp.modelData.workdir) }
                }
            }
            Repeater {
                model: grp.modelData.items
                delegate: Rectangle {
                    id: row
                    required property var modelData
                    readonly property var c: modelData
                    readonly property bool on: c.state === "running"
                    width: grp.width; height: 52
                    color: rowArea.containsMouse ? Theme.surface1 : "transparent"
                    MouseArea { id: rowArea; anchors.fill: parent; hoverEnabled: true; acceptedButtons: Qt.NoButton }
                    Rectangle {
                        x: 26; anchors.verticalCenter: parent.verticalCenter
                        width: 9; height: 9; radius: 4.5
                        color: row.on ? (row.c.status.includes("unhealthy") ? "#E3B341" : "#57AB5A") : row.c.state === "paused" ? "#539BF5" : Theme.surface2
                    }
                    Column {
                        x: 48; anchors.verticalCenter: parent.verticalCenter
                        width: parent.width - 48 - actions.width - 40
                        spacing: 2
                        Text { width: parent.width; elide: Text.ElideRight; text: row.c.service || row.c.name; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(12.5); weight: Font.Medium } }
                        Text {
                            width: parent.width; elide: Text.ElideRight
                            text: row.c.image + "  ·  " + row.c.status + (row.c.ports ? "  ·  " + row.c.ports : "")
                            color: Theme.faint
                            font { family: Theme.font; pointSize: Theme.pt(10.5) }
                        }
                    }
                    Row {
                        id: actions
                        anchors { right: parent.right; rightMargin: 24; verticalCenter: parent.verticalCenter }
                        spacing: 6
                        Button { visible: row.on; text: "Files"; onClicked: page.look(row.c, "/") }
                        Button { visible: row.on; text: "Shell"; onClicked: page.terminal("shell", row.c) }
                        Button { text: "Logs"; kind: "ghost"; onClicked: page.terminal("logs", row.c) }
                        Button {
                            text: row.on ? "Stop" : "Start"
                            kind: row.on ? "plain" : "primary"
                            enabled: !page.working()
                            onClicked: page.act({ what: row.on ? "stop" : "start", id: row.c.id }, (row.on ? "Stopped " : "Started ") + row.c.name)
                        }
                        IconButton {
                            width: 28; height: 28; icon: "menu"; iconSize: 13
                            onClicked: { more.c = row.c; more.openAt(0, height) }
                            Menu {
                                id: more
                                property var c: null
                                items: [
                                    { label: "Restart", enabled: !!more.c && more.c.state === "running", run: () => page.act({ what: "restart", id: more.c.id }, "Restarted " + more.c.name) },
                                    { label: more.c?.state === "paused" ? "Resume" : "Pause", enabled: !!more.c && (more.c.state === "running" || more.c.state === "paused"),
                                      run: () => page.act({ what: more.c.state === "paused" ? "unpause" : "pause", id: more.c.id }, "Done") },
                                    { separator: true },
                                    { label: "Remove…", danger: true, enabled: !!more.c && more.c.state !== "running", run: () => { ask.c = more.c; ask.open() } },
                                ]
                            }
                        }
                    }
                }
            }
        }
    }

    // ---- inside a container ----
    Text {
        visible: page.inside !== null && page.insideError !== ""
        anchors.centerIn: parent
        text: page.insideError
        color: Theme.rose
        font { family: Theme.font; pointSize: Theme.pt(12.5) }
    }
    ListView {
        id: files
        visible: page.inside !== null && page.insideError === ""
        anchors { left: parent.left; right: parent.right; top: ask.bottom; topMargin: 14; bottom: parent.bottom }
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
        model: page.entries
        delegate: Rectangle {
            id: fe
            required property var modelData
            width: files.width; height: 34
            color: feArea.containsMouse ? Theme.surface1 : "transparent"
            Icon { x: 26; anchors.verticalCenter: parent.verticalCenter; name: fe.modelData.dir ? "folder" : fe.modelData.link ? "link" : "file"; size: 15; color: fe.modelData.dir ? Theme.rose : Theme.muted }
            Text { x: 52; anchors.verticalCenter: parent.verticalCenter; width: parent.width - 260; elide: Text.ElideMiddle; text: fe.modelData.name; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(12) } }
            Text { anchors { right: copyBtn.left; rightMargin: 16; verticalCenter: parent.verticalCenter } visible: !fe.modelData.dir; text: F.size(fe.modelData.size); color: Theme.faint; font { family: Theme.mono; pointSize: Theme.pt(10.5) } }
            MouseArea {
                id: feArea
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: fe.modelData.dir ? Qt.PointingHandCursor : Qt.ArrowCursor
                onDoubleClicked: if (fe.modelData.dir) page.look(page.inside, page.join(page.at, fe.modelData.name))
                onClicked: if (fe.modelData.dir) page.look(page.inside, page.join(page.at, fe.modelData.name))
            }
            Button {
                id: copyBtn
                anchors { right: parent.right; rightMargin: 24; verticalCenter: parent.verticalCenter }
                text: "Copy out…"; kind: "ghost"
                onClicked: { copyTo.path = page.join(page.at, fe.modelData.name); copyTo.open() }
            }
        }
    }
}
