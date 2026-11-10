// Rename… with several selected: rules applied to every name (find and
// replace, with regular expressions; case; text before and after;
// numbering in the view's order; the extension), and a live preview of
// every new name. Problems (two the same, a name already taken, an empty
// name) are shown against the file and those files are left alone;
// nothing changes until Rename. The work is bulk_rename.rs.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files

Popup {
    id: root
    // [{ path, name, dir }] in the view's order.
    property var entries: []
    signal renamed(var paths)
    signal failed(string message)

    property string find: ""
    property string replace: ""
    property bool regex: false
    property bool matchCase: false
    property bool firstOnly: false
    property string textCase: "none"
    property string prefix: ""
    property string suffix: ""
    property bool number: false
    property string numberAt: "after"
    property string start: "1"
    property string step: "1"
    property int digits: 2
    property string separator: " "
    property bool wholeName: false
    property string extension: "keep"
    property string newExtension: ""

    // From the last preview.
    property var items: []
    property string error: ""
    readonly property int changing: items.filter(i => !i.problem && i.to !== i.from.substring(i.from.lastIndexOf("/") + 1)).length
    readonly property int problems: items.filter(i => i.problem).length

    readonly property var rules: ({
        find: root.find, replace: root.replace, regex: root.regex, match_case: root.matchCase, first_only: root.firstOnly,
        case: root.textCase, prefix: root.prefix, suffix: root.suffix,
        number: root.number, number_at: root.numberAt, start: parseInt(root.start) || 0, step: parseInt(root.step) || 0,
        digits: root.digits, separator: root.separator,
        whole_name: root.wholeName, extension: root.extension, new_extension: root.newExtension,
    })
    // Worked out again a moment after each change.
    onRulesChanged: if (opened) again.restart()
    Timer { id: again; interval: 120; onTriggered: root.refresh() }
    function args() { return [JSON.stringify(root.entries.map(e => e.path)), JSON.stringify(root.entries.map(e => !!e.dir)), JSON.stringify(root.rules)] }
    function refresh() {
        const r = JSON.parse(Ops.renamePreview(...root.args()));
        root.error = r.ok ? "" : r.error;
        if (r.ok) root.items = r.items;
    }

    function show(entries) {
        root.entries = entries;
        root.find = ""; root.replace = ""; root.prefix = ""; root.suffix = "";
        root.textCase = "none"; root.number = false; root.extension = "keep"; root.newExtension = "";
        root.items = [];
        root.open();
        root.refresh();
    }
    function go() {
        const r = JSON.parse(Ops.renameMany(...root.args()));
        root.close();
        if (r.failed.length > 0) root.failed(r.failed.length === 1 ? "Couldn't rename “" + r.failed[0][0].substring(r.failed[0][0].lastIndexOf("/") + 1) + "”: " + r.failed[0][1]
            : "Couldn't rename " + r.failed.length + " of them");
        root.renamed(r.done.map(d => d[1]));
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    width: Math.min(760, (parent?.width ?? 800) - 40)
    height: Math.min(680, (parent?.height ?? 700) - 40)
    modal: true
    padding: 20
    background: Rectangle { color: Theme.surface1; radius: 16; border { width: 1; color: Theme.surface2 } }
    Overlay.modal: Rectangle { color: Qt.rgba(0, 0, 0, 0.35) }
    onOpened: findField.focusField()

    component Label: Text {
        width: 96
        anchors.verticalCenter: parent.verticalCenter
        color: Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(12) }
    }
    // A small labelled switch.
    component Option: Row {
        id: opt
        property string text
        property bool checked
        signal toggled(bool on)
        spacing: 6
        anchors.verticalCenter: parent.verticalCenter
        Toggle { checked: opt.checked; onToggled: on => opt.toggled(on); anchors.verticalCenter: parent.verticalCenter }
        Text { text: opt.text; anchors.verticalCenter: parent.verticalCenter; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
    }

    contentItem: Column {
        spacing: 12
        Text {
            text: "Rename " + root.entries.length + " items"
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
        }
        Row {
            spacing: 10
            Label { text: "Find" }
            Field { id: findField; width: 200; mono: root.regex; placeholder: root.regex ? "Pattern, e.g. (\\d+)" : "Text"; text: root.find; onTextChanged: root.find = text; error: root.error !== "" }
            Text { text: "→"; anchors.verticalCenter: parent.verticalCenter; color: Theme.faint; font { family: Theme.font; pointSize: Theme.pt(13) } }
            Field { width: 200; mono: root.regex; placeholder: root.regex ? "Replace, $1 for a group" : "Replace with"; text: root.replace; onTextChanged: root.replace = text }
        }
        Row {
            spacing: 18
            x: 106
            Option { text: "Regular expression"; checked: root.regex; onToggled: on => root.regex = on }
            Option { text: "Match case"; checked: root.matchCase; onToggled: on => root.matchCase = on }
            Option { text: "First match only"; checked: root.firstOnly; onToggled: on => root.firstOnly = on }
        }
        Row {
            spacing: 10
            Label { text: "Case" }
            Segmented {
                value: root.textCase
                options: [{ label: "As is", value: "none" }, { label: "lower", value: "lower" }, { label: "UPPER", value: "upper" }, { label: "Title Case", value: "title" }, { label: "Sentence", value: "sentence" }]
                onPicked: v => root.textCase = v
            }
        }
        Row {
            spacing: 10
            Label { text: "Add" }
            Field { width: 160; placeholder: "Before"; text: root.prefix; onTextChanged: root.prefix = text }
            Text { text: "name"; anchors.verticalCenter: parent.verticalCenter; color: Theme.faint; font { family: Theme.font; pointSize: Theme.pt(11.5); italic: true } }
            Field { width: 160; placeholder: "After"; text: root.suffix; onTextChanged: root.suffix = text }
        }
        Row {
            spacing: 10
            Label { text: "Number" }
            Toggle { checked: root.number; onToggled: on => root.number = on; anchors.verticalCenter: parent.verticalCenter }
            Row {
                spacing: 8
                enabled: root.number
                opacity: root.number ? 1 : 0.4
                anchors.verticalCenter: parent.verticalCenter
                Choice {
                    value: root.numberAt
                    options: [{ label: "After the name", value: "after" }, { label: "Before the name", value: "before" }, { label: "Instead of the name", value: "replace" }]
                    onPicked: v => root.numberAt = v
                }
                Text { text: "from"; anchors.verticalCenter: parent.verticalCenter; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
                Field { width: 56; mono: true; text: root.start; onTextChanged: root.start = text }
                Text { text: "by"; anchors.verticalCenter: parent.verticalCenter; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
                Field { width: 44; mono: true; text: root.step; onTextChanged: root.step = text }
                Choice {
                    value: root.digits
                    options: [{ label: "1, 2, 3", value: 0 }, { label: "01", value: 2 }, { label: "001", value: 3 }, { label: "0001", value: 4 }]
                    onPicked: v => root.digits = v
                }
                Choice {
                    visible: root.numberAt !== "replace"
                    value: root.separator
                    options: [{ label: "Space", value: " " }, { label: "a - 1", value: " - " }, { label: "a-1", value: "-" }, { label: "a_1", value: "_" }, { label: "a1", value: "" }]
                    onPicked: v => root.separator = v
                }
            }
        }
        Row {
            spacing: 10
            Label { text: "Extension" }
            Choice {
                enabled: !root.wholeName
                value: root.extension
                options: [{ label: "Keep it", value: "keep" }, { label: "lowercase", value: "lower" }, { label: "UPPERCASE", value: "upper" }, { label: "Change to…", value: "set" }]
                onPicked: v => root.extension = v
            }
            Field { visible: root.extension === "set" && !root.wholeName; width: 90; mono: true; placeholder: "jpg"; text: root.newExtension; onTextChanged: root.newExtension = text }
            Option { text: "Rules include the extension"; checked: root.wholeName; onToggled: on => root.wholeName = on }
        }

        // ---- preview ----
        Rectangle {
            width: parent.width
            height: root.availableHeight - y - foot.height - 12
            radius: 10
            color: Theme.bg
            border { width: 1; color: Theme.surface2 }
            Text {
                visible: root.error !== ""
                anchors.centerIn: parent
                width: parent.width - 40
                horizontalAlignment: Text.AlignHCenter
                wrapMode: Text.Wrap
                text: root.error
                color: Theme.rose
                font { family: Theme.font; pointSize: Theme.pt(12) }
            }
            ListView {
                id: list
                visible: root.error === ""
                anchors { fill: parent; margins: 6 }
                clip: true
                model: root.items
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                delegate: Item {
                    id: row
                    required property var modelData
                    readonly property string old: modelData.from.substring(modelData.from.lastIndexOf("/") + 1)
                    readonly property bool same: modelData.to === old
                    width: list.width - 8
                    height: modelData.problem ? 44 : 28
                    Text {
                        id: oldText
                        x: 8; y: 6
                        width: (parent.width - 40) / 2
                        elide: Text.ElideMiddle
                        text: row.old
                        color: row.same ? Theme.faint : Theme.muted
                        font { family: Theme.font; pointSize: Theme.pt(11.5) }
                    }
                    Text { x: oldText.x + oldText.width + 6; y: 6; text: "→"; color: Theme.faint; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
                    Text {
                        x: oldText.x + oldText.width + 26; y: 6
                        width: (parent.width - 40) / 2
                        elide: Text.ElideMiddle
                        text: row.same ? "(unchanged)" : row.modelData.to
                        color: row.modelData.problem ? Theme.rose : row.same ? Theme.faint : Theme.text
                        font { family: Theme.font; pointSize: Theme.pt(11.5); weight: row.same ? Font.Normal : Font.Medium }
                    }
                    Text {
                        visible: !!row.modelData.problem
                        x: oldText.x + oldText.width + 26; y: 24
                        width: (parent.width - 40) / 2
                        elide: Text.ElideRight
                        text: row.modelData.problem
                        color: Theme.rose
                        font { family: Theme.font; pointSize: Theme.pt(10.5) }
                    }
                }
            }
        }
        Item {
            id: foot
            width: parent.width; height: 34
            Text {
                anchors { left: parent.left; verticalCenter: parent.verticalCenter }
                text: root.error !== "" ? "" : (root.changing + " will be renamed") + (root.problems > 0 ? " · " + root.problems + " left as they are (see why above)" : "")
                color: root.problems > 0 ? Theme.rose : Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(11.5) }
            }
            Row {
                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                spacing: 8
                Button { text: "Cancel"; onClicked: root.close() }
                Button {
                    text: root.changing > 0 ? "Rename " + root.changing : "Rename"
                    kind: "primary"
                    enabled: root.error === "" && root.changing > 0
                    onClicked: root.go()
                }
            }
        }
    }
}
