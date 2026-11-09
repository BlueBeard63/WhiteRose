// Find (Ctrl+F) and replace (Ctrl+H) in the current document. Enter finds
// the next match, Shift+Enter the previous, Esc closes.
import QtQuick
import WhiteRose.Kit

Rectangle {
    id: bar
    property var editor: null
    property bool replacing: false
    property bool matchCase: false
    readonly property string query: findField.text
    color: Theme.surface1
    height: visible ? col.implicitHeight + 16 : 0
    visible: false

    function open(replace) {
        bar.replacing = replace;
        bar.visible = true;
        const sel = editor?.area.selectedText ?? "";
        if (sel !== "" && sel.indexOf("\n") < 0) findField.text = sel;
        findField.focusField();
        findField.selectAll();
    }
    function close() { bar.visible = false; editor?.area.forceActiveFocus() }

    // Every match's start (simple text search; fine for files up to 20 MiB).
    readonly property var matches: {
        if (!editor || query === "") return [];
        const hay = matchCase ? editor.text : editor.text.toLowerCase();
        const needle = matchCase ? query : query.toLowerCase();
        const out = [];
        for (let i = hay.indexOf(needle); i >= 0 && out.length < 10000; i = hay.indexOf(needle, i + Math.max(1, needle.length))) out.push(i);
        return out;
    }
    readonly property int current: {
        if (!editor || matches.length === 0) return -1;
        const at = editor.area.selectionStart;
        const i = matches.indexOf(at);
        return i;
    }
    function select(i) {
        if (matches.length === 0) return;
        const start = matches[(i + matches.length) % matches.length];
        editor.area.select(start, start + query.length);
    }
    function next() {
        if (matches.length === 0) return;
        const from = editor.area.selectionEnd;
        const i = matches.findIndex(m => m >= from);
        select(i < 0 ? 0 : i);
    }
    function prev() {
        if (matches.length === 0) return;
        const from = editor.area.selectionStart;
        let i = -1;
        for (let k = matches.length - 1; k >= 0; k--) if (matches[k] < from) { i = k; break }
        select(i < 0 ? matches.length - 1 : i);
    }
    function replaceOne() {
        if (current < 0) { next(); return }
        const s = editor.area.selectionStart;
        editor.area.remove(s, s + query.length);
        editor.area.insert(s, replaceField.text);
        next();
    }
    function replaceAll() {
        if (matches.length === 0) return;
        // Back to front, so earlier positions stay right.
        for (let k = matches.length - 1; k >= 0; k--) {
            editor.area.remove(matches[k], matches[k] + query.length);
            editor.area.insert(matches[k], replaceField.text);
        }
    }

    Rectangle { width: parent.width; height: 1; color: Theme.surface2 }
    Column {
        id: col
        x: 12; y: 8
        width: parent.width - 24
        spacing: 6
        Row {
            spacing: 8
            Field {
                id: findField
                implicitWidth: 280; implicitHeight: 32
                placeholder: "Find"
                onAccepted: bar.next()
                Keys.onEscapePressed: bar.close()
            }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                width: 74
                text: bar.query === "" ? "" : bar.matches.length === 0 ? "No matches" : (bar.current >= 0 ? (bar.current + 1) + " of " : "") + bar.matches.length
                color: bar.query !== "" && bar.matches.length === 0 ? Theme.rose : Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(11.5) }
            }
            IconButton { width: 30; height: 30; icon: "chevUp"; onClicked: bar.prev() }
            IconButton { width: 30; height: 30; icon: "chevDown"; onClicked: bar.next() }
            Rectangle {
                anchors.verticalCenter: parent.verticalCenter
                width: caseText.implicitWidth + 16; height: 28; radius: 8
                color: bar.matchCase ? Theme.pearl : Theme.surface2
                Text { id: caseText; anchors.centerIn: parent; text: "Aa"; color: bar.matchCase ? Theme.bg : Theme.muted; font { family: Theme.font; pointSize: Theme.pt(12); weight: Font.Medium } }
                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: bar.matchCase = !bar.matchCase }
            }
            IconButton { width: 30; height: 30; icon: "x"; onClicked: bar.close() }
        }
        Row {
            visible: bar.replacing
            spacing: 8
            Field {
                id: replaceField
                implicitWidth: 280; implicitHeight: 32
                placeholder: "Replace with"
                onAccepted: bar.replaceOne()
                Keys.onEscapePressed: bar.close()
            }
            Button { text: "Replace"; enabled: bar.matches.length > 0; onClicked: bar.replaceOne() }
            Button { text: "Replace all"; enabled: bar.matches.length > 0; onClicked: bar.replaceAll() }
        }
    }
    Keys.onPressed: e => { if (e.key === Qt.Key_Return && (e.modifiers & Qt.ShiftModifier)) { bar.prev(); e.accepted = true } }
}
