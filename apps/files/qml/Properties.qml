// Properties of what's selected (or of the folder you're in): what it is,
// where, how big (folders counted in the background as you watch), when,
// and, for one thing of yours, who may read, change and run it (changed
// straight away). Several things: how many, and their size together.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files
import "Format.js" as F

Popup {
    id: root
    property var paths: []
    property var info: ({})
    property var count: ({ bytes: 0, files: 0, folders: 0, done: true })
    property int measuring: -1
    property string error: ""
    readonly property bool one: root.paths.length === 1
    readonly property bool canChange: root.one && root.info.mine === true && !root.info.link && !root.info.origin

    function show(paths) {
        root.paths = paths;
        root.info = paths.length === 1 ? JSON.parse(Fs.info(paths[0])) : {};
        if (paths.length === 0 || (paths.length === 1 && !root.info.ok)) return;
        root.error = "";
        root.count = { bytes: 0, files: 0, folders: 0, done: false };
        root.measuring = Ops.measure(JSON.stringify(paths));
        root.open();
    }
    function setBit(who, what, on) {
        const mode = F.withBit(root.info.mode, who, what, on);
        const err = Fs.setMode(root.info.path, mode);
        root.error = err;
        if (err === "") root.info = JSON.parse(Fs.info(root.info.path));
    }

    Connections {
        target: Ops
        function onMeasured(id, result) { if (id === root.measuring) root.count = JSON.parse(result) }
    }
    onClosed: { Ops.stopMeasuring(root.measuring); root.measuring = -1 }

    parent: Overlay.overlay
    anchors.centerIn: parent
    width: 440
    modal: true
    focus: true
    padding: 22
    background: Rectangle { color: Theme.surface1; radius: 16; border { width: 1; color: Theme.surface2 } }
    Overlay.modal: Rectangle { color: Qt.rgba(0, 0, 0, 0.35) }

    component Line: Row {
        property string label
        property string value
        visible: value !== ""
        width: parent.width
        spacing: 12
        Text { width: 110; text: parent.label; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(12) } }
        Text {
            width: parent.width - 122
            text: parent.value
            wrapMode: Text.WrapAnywhere
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(12) }
        }
    }

    contentItem: Column {
        spacing: 14

        // ---- what it is -------------------------------------------------------------
        Row {
            spacing: 14
            width: parent.width
            Image {
                width: 48; height: 48
                sourceSize: Qt.size(96, 96)
                source: root.one && root.info.icon ? "file://" + root.info.icon : ""
                visible: root.one
            }
            Icon { visible: !root.one; name: "copy"; size: 40; color: Theme.muted }
            Column {
                anchors.verticalCenter: parent.verticalCenter
                width: parent.width - 62
                spacing: 2
                Text {
                    width: parent.width
                    text: root.one ? (root.info.name ?? "") : root.paths.length + " items"
                    elide: Text.ElideMiddle
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
                }
                Text {
                    visible: root.one
                    text: root.info.link ? "Link" : F.kind(root.info.mime ?? "") + (root.info.mime && !root.info.dir ? " · " + root.info.mime : "")
                    color: Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(11.5) }
                }
            }
        }

        Rectangle { width: parent.width; height: 1; color: Theme.surface2 }

        // ---- where, how big, when -------------------------------------------------------
        Column {
            width: parent.width
            spacing: 8
            Line {
                label: "Size"
                value: {
                    const c = root.count;
                    if (root.one && !root.info.dir) return F.exactSize(root.info.size ?? 0);
                    const parts = [];
                    if (c.files > 0 || c.done) parts.push(c.files + (c.files === 1 ? " file" : " files"));
                    if (c.folders > 0) parts.push(c.folders + (c.folders === 1 ? " folder" : " folders"));
                    return parts.join(", ") + (c.bytes > 0 || c.done ? ", " + F.size(c.bytes) : "") + (c.done ? "" : "…");
                }
            }
            Line { label: "Location"; value: root.one && !root.info.origin ? String(root.info.location ?? "").replace(Fs.home(), "~") : "" }
            Line { label: "Link to"; value: root.one && root.info.target ? root.info.target : "" }
            Line { label: "From"; value: root.one && root.info.origin ? String(root.info.origin).replace(Fs.home(), "~") : "" }
            Line { label: "Deleted"; value: root.one ? F.fullDate(root.info.deleted ?? 0) : "" }
            Line { label: "Modified"; value: root.one ? F.fullDate(root.info.modified) : "" }
            Line { label: "Opened"; value: root.one ? F.fullDate(root.info.accessed) : "" }
            Line { label: "Created"; value: root.one ? F.fullDate(root.info.created) : "" }
        }

        // ---- who may do what ----------------------------------------------------------------
        Rectangle { visible: root.one && !root.info.origin; width: parent.width; height: 1; color: Theme.surface2 }
        Column {
            visible: root.one && !root.info.origin
            width: parent.width
            spacing: 8
            Line { label: "Owner"; value: root.info.owner ?? "" }
            Line { label: "Group"; value: root.info.group ?? "" }
            // Who × read, change, run (folders: open).
            Row {
                spacing: 0
                Item { width: 122; height: 1 }
                Repeater {
                    model: ["Read", "Change", root.info.dir ? "Open" : "Run"]
                    delegate: Text {
                        required property string modelData
                        width: 70
                        text: modelData
                        color: Theme.faint
                        font { family: Theme.font; pointSize: Theme.pt(10.5) }
                    }
                }
            }
            Repeater {
                model: ["You (" + (root.info.owner ?? "") + ")", "Group", "Everyone"]
                delegate: Row {
                    required property string modelData
                    required property int index
                    readonly property int who: index
                    Text {
                        width: 122
                        anchors.verticalCenter: parent.verticalCenter
                        text: index === 0 && !root.info.mine ? "Owner" : modelData
                        elide: Text.ElideRight
                        color: Theme.muted
                        font { family: Theme.font; pointSize: Theme.pt(12) }
                    }
                    Repeater {
                        model: 3
                        delegate: Item {
                            required property int index
                            width: 70; height: 26
                            Toggle {
                                anchors.left: parent.left
                                checked: F.hasBit(root.info.mode ?? 0, parent.parent.who, index)
                                enabled: root.canChange
                                onToggled: on => root.setBit(parent.parent.who, index, on)
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
                font { family: Theme.font; pointSize: Theme.pt(11.5) }
            }
        }

        Row {
            anchors.right: parent.right
            Button { text: "Close"; kind: "primary"; onClicked: root.close() }
        }
    }
}
