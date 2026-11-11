// The Sheet's Rename tab: rules applied to every selected name (find and
// replace, with patterns; case; text before and after; numbering in the
// view's order; the extension). The new names show in the list itself as
// you type ("old → new"), problems (two the same, a name already taken, an
// empty one) against the file, which is left alone. Nothing changes until
// Rename. The work is bulk_rename.rs.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files

Column {
    id: root
    property var pane: null
    // Shown (the Sheet's tab is Rename): the list shows the new names.
    property bool active: false
    signal renamed(var paths)
    signal failed(string message)
    signal cancelled()

    // [{ path, dir }] in the view's order.
    readonly property var entries: (root.pane?.selection ?? []).map(e => ({ path: e.path, dir: !!e.dir }))
    readonly property bool typing: findField.editing || replaceField.editing || prefixField.editing || suffixField.editing || startField.editing || stepField.editing || extField.editing

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
    // Worked out again a moment after each change (rules or selection).
    onRulesChanged: if (root.active) again.restart()
    onEntriesChanged: if (root.active) again.restart()
    onActiveChanged: { if (root.active) { root.reset(); root.refresh(); findField.focusField() } else root.show([]) }
    Timer { id: again; interval: 120; onTriggered: root.refresh() }
    function args() { return [JSON.stringify(root.entries.map(e => e.path)), JSON.stringify(root.entries.map(e => e.dir)), JSON.stringify(root.rules)] }
    function refresh() {
        if (root.entries.length === 0) { root.items = []; root.show([]); return }
        const r = JSON.parse(Ops.renamePreview(...root.args()));
        root.error = r.ok ? "" : r.error;
        if (r.ok) root.items = r.items;
        root.show(r.ok ? r.items : []);
    }
    // The new names, for the list: {path: {to, problem}}.
    function show(items) {
        if (!root.pane) return;
        const m = {};
        for (const i of items) m[i.from] = { to: i.to, problem: i.problem ?? "" };
        root.pane.renaming = m;
    }
    function reset() {
        root.find = ""; root.replace = ""; root.prefix = ""; root.suffix = "";
        root.textCase = "none"; root.number = false; root.extension = "keep"; root.newExtension = "";
        findField.text = ""; replaceField.text = ""; prefixField.text = ""; suffixField.text = ""; extField.text = "";
        root.items = []; root.error = "";
    }
    function go() {
        const r = JSON.parse(Ops.renameMany(...root.args()));
        root.show([]);
        if (r.failed.length > 0) root.failed(r.failed.length === 1 ? "Couldn't rename “" + r.failed[0][0].substring(r.failed[0][0].lastIndexOf("/") + 1) + "”: " + r.failed[0][1]
            : "Couldn't rename " + r.failed.length + " of them");
        root.renamed(r.done.map(d => d[1]));
        root.reset();
    }

    spacing: 12

    // ---- pieces ----
    component Label: Text {
        color: Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(10.5) }
    }
    // A setting row: its name, and a switch at the right.
    component SwitchRow: Item {
        id: sw
        property string text
        property bool checked
        signal toggled(bool on)
        width: parent.width; height: 30
        Text { anchors.verticalCenter: parent.verticalCenter; text: sw.text; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(12) } }
        Toggle { anchors { right: parent.right; verticalCenter: parent.verticalCenter } label: sw.text; checked: sw.checked; onToggled: on => sw.toggled(on) }
    }
    // A row: its name at the left, what sets it at the right.
    component Line: Item {
        default property alias content: right.data
        property string text
        width: parent.width; height: 34
        Text { anchors.verticalCenter: parent.verticalCenter; text: parent.text; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(12) } }
        Row { id: right; anchors { right: parent.right; verticalCenter: parent.verticalCenter } spacing: 6 }
    }

    Text {
        width: parent.width
        elide: Text.ElideRight
        text: root.entries.length === 1 ? "Rename 1 item" : "Rename " + root.entries.length + " items"
        color: Theme.text
        font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
    }
    Text {
        visible: root.entries.length === 0
        width: parent.width
        wrapMode: Text.Wrap
        text: "Choose what to rename in the list."
        color: Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(12) }
    }

    Column {
        visible: root.entries.length > 0
        width: parent.width
        spacing: 4
        Label { text: "Find" }
        Field { id: findField; label: "Find"; Keys.onTabPressed: replaceField.focusField(); width: parent.width; height: 34; mono: true; placeholder: root.regex ? "Pattern, e.g. (\\d+)" : "Text in the names"; onTextChanged: root.find = text; error: root.error !== "" }
    }
    Column {
        visible: root.entries.length > 0
        width: parent.width
        spacing: 4
        Label { text: "Replace with" }
        Field { id: replaceField; label: "Replace with"; Keys.onTabPressed: prefixField.focusField(); Keys.onBacktabPressed: findField.focusField(); width: parent.width; height: 34; mono: true; placeholder: root.regex ? "$1 for a group" : "Nothing (take it out)"; onTextChanged: root.replace = text }
    }
    Column {
        visible: root.entries.length > 0
        width: parent.width
        SwitchRow { text: "Pattern (regular expression)"; checked: root.regex; onToggled: on => root.regex = on }
        SwitchRow { text: "Match case"; checked: root.matchCase; onToggled: on => root.matchCase = on }
        SwitchRow { text: "First match only"; checked: root.firstOnly; onToggled: on => root.firstOnly = on }
    }
    Column {
        visible: root.entries.length > 0
        width: parent.width
        Line {
            text: "Case"
            Choice {
                value: root.textCase
                options: [{ label: "As is", value: "none" }, { label: "lower", value: "lower" }, { label: "UPPER", value: "upper" }, { label: "Title Case", value: "title" }, { label: "Sentence", value: "sentence" }]
                onPicked: v => root.textCase = v
            }
        }
        Line {
            text: "Add"
            Field { id: prefixField; Keys.onTabPressed: suffixField.focusField(); Keys.onBacktabPressed: replaceField.focusField(); width: 74; height: 30; placeholder: "before"; onTextChanged: root.prefix = text }
            Field { id: suffixField; Keys.onTabPressed: findField.focusField(); Keys.onBacktabPressed: prefixField.focusField(); width: 74; height: 30; placeholder: "after"; onTextChanged: root.suffix = text }
        }
        SwitchRow { text: "Number them"; checked: root.number; onToggled: on => root.number = on }
        Line {
            visible: root.number
            text: ""
            Segmented {
                value: root.numberAt
                options: [{ label: "After", value: "after" }, { label: "Before", value: "before" }, { label: "Instead of", value: "replace" }]
                onPicked: v => root.numberAt = v
            }
        }
        Line {
            visible: root.number
            text: ""
            Text { text: "from"; anchors.verticalCenter: parent.verticalCenter; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
            Field { id: startField; width: 40; height: 30; mono: true; text: "1"; onTextChanged: root.start = text }
            Text { text: "by"; anchors.verticalCenter: parent.verticalCenter; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
            Field { id: stepField; width: 34; height: 30; mono: true; text: "1"; onTextChanged: root.step = text }
            Choice {
                value: root.digits
                options: [{ label: "1", value: 0 }, { label: "01", value: 2 }, { label: "001", value: 3 }, { label: "0001", value: 4 }]
                onPicked: v => root.digits = v
            }
        }
        Line {
            visible: root.number && root.numberAt !== "replace"
            text: "Between"
            Choice {
                value: root.separator
                options: [{ label: "a space", value: " " }, { label: "a - 1", value: " - " }, { label: "a-1", value: "-" }, { label: "a_1", value: "_" }, { label: "nothing", value: "" }]
                onPicked: v => root.separator = v
            }
        }
        Line {
            text: "Extension"
            Field { id: extField; visible: root.extension === "set" && !root.wholeName; width: 60; height: 30; mono: true; placeholder: "jpg"; onTextChanged: root.newExtension = text }
            Choice {
                enabled: !root.wholeName
                value: root.extension
                options: [{ label: "Keep", value: "keep" }, { label: "lowercase", value: "lower" }, { label: "UPPERCASE", value: "upper" }, { label: "Change…", value: "set" }]
                onPicked: v => root.extension = v
            }
        }
        SwitchRow { text: "Rules include the extension"; checked: root.wholeName; onToggled: on => root.wholeName = on }
    }

    // ---- what will happen (the names themselves are in the list) ----
    Text {
        visible: root.entries.length > 0
        width: parent.width
        wrapMode: Text.Wrap
        text: root.error !== "" ? root.error
            : (root.changing === 1 ? "1 renamed" : root.changing + " renamed") + (root.problems > 0 ? " · " + root.problems + " left as " + (root.problems === 1 ? "it is" : "they are") + " (see the list)" : "")
        color: root.error !== "" || root.problems > 0 ? Theme.rose : Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(11) }
    }
    // Not in the list view: the names here instead.
    Column {
        visible: root.entries.length > 0 && root.pane?.mode !== "list" && root.error === ""
        width: parent.width
        spacing: 3
        Repeater {
            model: root.items.slice(0, 12)
            delegate: Text {
                required property var modelData
                width: parent.width
                elide: Text.ElideMiddle
                text: modelData.from.substring(modelData.from.lastIndexOf("/") + 1) + "  →  " + modelData.to
                color: modelData.problem ? Theme.rose : Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(10.5) }
            }
        }
    }
    Row {
        visible: root.entries.length > 0
        spacing: 8
        Button { anchors.verticalCenter: undefined; text: "Cancel"; kind: "ghost"; onClicked: { root.reset(); root.refresh(); root.cancelled() } }
        Button {
            anchors.verticalCenter: undefined
            text: root.changing > 0 ? "Rename " + root.changing : "Rename"
            kind: "primary"
            enabled: root.error === "" && root.changing > 0
            onClicked: root.go()
        }
    }
}
