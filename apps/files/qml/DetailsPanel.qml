// Beside the folder (Alt+P): the selected file previewed, with what it is.
// Several selected: how many and their size; none: the folder itself, and
// if it's a project (Cargo, npm, Go, Make…), its scripts to run in your
// terminal (projects.rs). In an untrusted place (Downloads…) running asks
// first: a project's scripts run whatever it says.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F

Rectangle {
    id: root
    property var pane: null
    readonly property var sel: root.pane?.selection ?? []
    readonly property var entry: root.sel.length === 1 ? root.sel[0] : null
    readonly property real now: Date.now()
    // A script to run: Main runs it in the chosen terminal.
    signal runScript(string dir, string command)
    // The folder's projects (looked at when it changes).
    property var projects: ({ projects: [], why: "" })
    property bool trusted: false
    function lookAtProjects() {
        root.trusted = false;
        root.projects = root.pane && root.pane.path.startsWith("/") && !root.pane.special ? JSON.parse(Fs.projects(root.pane.path)) : { projects: [], why: "" };
    }
    Connections { target: root.pane; function onPathChanged() { root.lookAtProjects() } }
    onPaneChanged: lookAtProjects()
    color: Theme.bg
    Rectangle { width: 1; height: parent.height; color: Theme.surface1 }

    component Fact: Column {
        property string label
        property string value
        visible: value !== ""
        width: parent.width
        spacing: 1
        Text { text: parent.label; color: Theme.faint; font { family: Theme.font; pointSize: Theme.pt(10.5) } }
        Text {
            width: parent.width
            text: parent.value
            wrapMode: Text.WrapAnywhere
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(12) }
        }
    }

    Flickable {
        anchors { fill: parent; leftMargin: 1 }
        contentHeight: col.implicitHeight + 32
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        Column {
            id: col
            x: 16; y: 16
            width: parent.width - 32
            spacing: 14

            // ---- one file ------------------------------------------------------------
            Rectangle {
                visible: root.entry !== null
                width: parent.width; height: Math.round(width * 0.8)
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
            Column {
                visible: root.entry !== null
                width: parent.width
                spacing: 10
                Text {
                    width: parent.width
                    text: root.entry?.name ?? ""
                    wrapMode: Text.WrapAnywhere
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(13.5); weight: Font.DemiBold }
                }
                Fact { label: "Kind"; value: root.entry ? (root.entry.dir ? "Folder" : F.kind(root.entry.mime)) : "" }
                Fact {
                    label: preview.kind === "image" ? "Dimensions" : preview.kind === "pdf" ? "Pages" : preview.kind === "video" || preview.kind === "audio" ? "Length" : ""
                    value: label !== "" ? preview.facts : ""
                }
                Fact { label: "Size"; value: root.entry && !root.entry.dir ? F.exactSize(root.entry.size) : "" }
                // Its tags (click one: every file with it).
                Column {
                    visible: (root.entry?.tags ?? []).length > 0
                    width: parent.width
                    spacing: 4
                    Text { text: "Tags"; color: Theme.faint; font { family: Theme.font; pointSize: Theme.pt(10.5) } }
                    Flow {
                        width: parent.width
                        spacing: 6
                        Repeater {
                            model: root.entry?.tags ?? []
                            delegate: Rectangle {
                                required property string modelData
                                height: 24; radius: 12
                                width: chipRow.implicitWidth + 16
                                color: chipArea.containsMouse ? Theme.surface2 : Theme.surface1
                                Row {
                                    id: chipRow
                                    anchors.centerIn: parent
                                    spacing: 6
                                    Rectangle { width: 8; height: 8; radius: 4; color: Session.tagColour(modelData); anchors.verticalCenter: parent.verticalCenter }
                                    Text { text: modelData; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(11) } }
                                }
                                MouseArea { id: chipArea; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: root.pane.go("tag:" + modelData) }
                            }
                        }
                    }
                }
                Fact { label: "Found"; value: root.entry?.found ?? "" }
                Fact { label: root.pane?.inTrash ? "Deleted" : "Modified"; value: root.entry ? F.fullDate(root.entry.modified) : "" }
                Fact { label: root.pane?.inTrash ? "From" : "Folder"; value: String(root.entry?.origin ?? root.entry?.folder ?? "").replace(Fs.home(), "~") }
                Button { text: "Open"; kind: "primary"; anchors.verticalCenter: undefined; onClicked: root.pane.activate(root.entry) }
            }

            // ---- several, or none (the folder) ------------------------------------------------
            Column {
                visible: root.entry === null
                width: parent.width
                spacing: 10
                Icon { name: root.sel.length > 1 ? "copy" : root.pane?.searching ? "search" : root.pane?.inDrives ? "gauge" : root.pane?.inTrash ? "trash" : "folder"; size: 40; color: Theme.muted }
                Text {
                    width: parent.width
                    text: root.sel.length > 1 ? root.sel.length + " items" : root.pane?.title ?? ""
                    wrapMode: Text.WrapAnywhere
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(13.5); weight: Font.DemiBold }
                }
                Fact {
                    label: "Size"
                    value: {
                        const files = root.sel.filter(e => !e.dir);
                        return root.sel.length > 1 && files.length > 0 ? F.size(files.reduce((a, e) => a + e.size, 0)) + (files.length < root.sel.length ? " (files only)" : "") : "";
                    }
                }
                Fact {
                    label: "Contains"
                    value: root.sel.length === 0 && root.pane ? root.pane.shownEntries.length + (root.pane.shownEntries.length === 1 ? " item" : " items") : ""
                }
                // ---- a project's scripts ----
                Column {
                    visible: root.sel.length === 0 && root.projects.projects.length > 0
                    width: parent.width
                    spacing: 6
                    Text { text: "Scripts"; color: Theme.faint; font { family: Theme.font; pointSize: Theme.pt(10.5); weight: Font.Medium; capitalization: Font.AllUppercase; letterSpacing: 0.6 } }
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
                        Button { id: trust; x: 10; y: warn.y + warn.height + 6; text: "I trust this project"; kind: "danger"; onClicked: root.trusted = true }
                    }
                    Repeater {
                        model: root.projects.projects
                        delegate: Column {
                            id: proj
                            required property var modelData
                            width: parent.width
                            spacing: 2
                            Text { text: proj.modelData.kind + "  ·  " + proj.modelData.file; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11); weight: Font.Medium } }
                            Repeater {
                                model: proj.modelData.scripts
                                delegate: Rectangle {
                                    id: sc
                                    required property var modelData
                                    readonly property bool can: root.projects.why === "" || root.trusted
                                    width: parent.width; height: 30; radius: 8
                                    color: scArea.containsMouse && sc.can ? Theme.surface1 : "transparent"
                                    Icon { x: 6; anchors.verticalCenter: parent.verticalCenter; name: "play"; size: 11; color: sc.can ? Theme.rose : Theme.faint }
                                    Text {
                                        x: 24; anchors.verticalCenter: parent.verticalCenter
                                        width: parent.width - 30
                                        elide: Text.ElideRight
                                        text: sc.modelData.name
                                        color: sc.can ? Theme.text : Theme.faint
                                        font { family: Theme.font; pointSize: Theme.pt(11.5) }
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
    }
}
