// One document: the scratch note or a file. Line numbers (when lines don't
// wrap), find support, autosave for the scratch note, and noticing when the
// file changes on disk (reloaded if you haven't edited it, else asked).
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Notepad

Item {
    id: ed
    property string path: ""          // "" until a new file is saved
    // The scratch note (always the first tab; saves itself).
    property bool scratch: false
    property string eol: "lf"
    property real savedMtime: 0
    property bool dirty: false
    property bool wrap: true
    property string language: "Plain text"
    property string syntax: ""
    // Set when the file changed on disk while you had unsaved edits.
    property bool changedOnDisk: false
    property string error: ""
    readonly property alias text: area.text
    readonly property alias area: area
    property bool loading: false

    readonly property string title: scratch ? "Scratch" : path === "" ? "Untitled" : path.substring(path.lastIndexOf("/") + 1)

    function load() {
        if (scratch) {
            ed.loading = true;
            area.text = Files.scratch();
            ed.loading = false;
            ed.wrap = true;
            ed.language = "Markdown";
            ed.syntax = "Markdown";
            return true;
        }
        const r = JSON.parse(Files.open(path));
        if (!r.ok) { ed.error = r.error; return false; }
        ed.loading = true;
        area.text = r.text;
        ed.loading = false;
        ed.eol = r.eol;
        ed.savedMtime = r.mtime;
        ed.dirty = false;
        ed.changedOnDisk = false;
        const l = JSON.parse(Files.language(path));
        ed.language = l.name;
        ed.syntax = l.syntax;
        // Prose wraps; code doesn't (so line numbers line up).
        ed.wrap = l.syntax === "" || l.syntax === "Markdown";
        return true;
    }

    // Saves to `to` (default: its own path). Returns "" or the error.
    function save(to) {
        const target = to || path;
        if (target === "") return "no file name";
        const r = JSON.parse(Files.save(target, area.text, eol));
        if (!r.ok) return r.error;
        if (target !== path) {
            ed.path = target;
            const l = JSON.parse(Files.language(path));
            ed.language = l.name;
            ed.syntax = l.syntax;
        }
        ed.savedMtime = r.mtime;
        ed.dirty = false;
        ed.changedOnDisk = false;
        return "";
    }

    function goToLine(n) {
        const lines = area.text.split("\n");
        n = Math.max(1, Math.min(lines.length, n));
        let pos = 0;
        for (let i = 0; i < n - 1; i++) pos += lines[i].length + 1;
        area.cursorPosition = pos;
        area.forceActiveFocus();
    }

    // The scratch note saves itself a moment after you stop typing.
    Timer { id: autosave; interval: 700; onTriggered: Files.saveScratch(area.text) }

    // Changed on disk? Checked every 2 s while Notepad is in front.
    Timer {
        interval: 2000
        repeat: true
        running: !ed.scratch && ed.path !== "" && Qt.application.state === Qt.ApplicationActive
        onTriggered: {
            const m = Files.mtime(ed.path);
            if (m === 0 || m <= ed.savedMtime) return;
            if (!ed.dirty) {
                const at = area.cursorPosition;
                ed.load();
                area.cursorPosition = Math.min(at, area.length);
            } else ed.changedOnDisk = true;
        }
    }

    FontMetrics { id: metrics; font: area.font }

    // Where each line starts (character offsets), kept up to date a moment
    // after edits; line numbers are placed at these lines' real positions.
    property var lineStarts: [0]
    function indexLines() {
        const t = area.text, starts = [0];
        for (let i = t.indexOf("\n"); i >= 0; i = t.indexOf("\n", i + 1)) starts.push(i + 1);
        ed.lineStarts = starts;
    }
    Timer { id: reindex; interval: 120; onTriggered: ed.indexLines() }
    Connections { target: area; function onTextChanged() { reindex.restart() } }

    // ---- line numbers ---------------------------------------------------------
    Rectangle {
        id: gutter
        visible: win.lineNumbers
        width: visible ? Math.max(3, String(ed.lineStarts.length).length) * metrics.averageCharacterWidth + 26 : 0
        anchors { left: parent.left; top: parent.top; bottom: parent.bottom }
        color: Theme.bg
        clip: true
        // The first line on screen: a binary search over real positions.
        readonly property int first: {
            const starts = ed.lineStarts, top = flick.contentY;
            void area.contentHeight; void flick.width;
            let lo = 0, hi = starts.length - 1;
            while (lo < hi) {
                const mid = (lo + hi + 1) >> 1;
                if (area.positionToRectangle(starts[mid]).y <= top) lo = mid; else hi = mid - 1;
            }
            return lo;
        }
        readonly property var shown: {
            const out = [], starts = ed.lineStarts, bottom = flick.contentY + flick.height;
            void area.contentHeight; void flick.width;
            for (let i = gutter.first; i < starts.length && out.length < 400; i++) {
                const y = area.positionToRectangle(starts[i]).y;
                if (y > bottom) break;
                out.push({ n: i + 1, y: y });
            }
            return out;
        }
        Repeater {
            model: gutter.shown
            delegate: Text {
                required property var modelData
                x: 0; width: gutter.width - 14
                y: modelData.y - flick.contentY
                height: metrics.height
                horizontalAlignment: Text.AlignRight
                text: modelData.n
                color: modelData.n === status.pos.line ? Theme.muted : Theme.faint
                font: area.font
            }
        }
    }

    // ---- the text --------------------------------------------------------------------
    Flickable {
        id: flick
        anchors { left: gutter.right; right: parent.right; top: parent.top; bottom: parent.bottom }
        contentWidth: ed.wrap ? width : Math.max(width, area.contentWidth + area.leftPadding + area.rightPadding)
        contentHeight: area.contentHeight + area.topPadding + area.bottomPadding
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
        ScrollBar.horizontal: ScrollBar { policy: ScrollBar.AsNeeded }

        // Keep the cursor in view (the editor never scrolls by itself).
        function ensureVisible(r) {
            const margin = 24;
            if (contentX >= r.x - margin) contentX = Math.max(0, r.x - margin);
            else if (contentX + width <= r.x + r.width + margin) contentX = r.x + r.width + margin - width;
            if (contentY >= r.y) contentY = Math.max(0, r.y - 8);
            else if (contentY + height <= r.y + r.height) contentY = r.y + r.height - height + 8;
        }

        TextEdit {
            id: area
            width: ed.wrap ? flick.width : Math.max(flick.width, contentWidth + leftPadding + rightPadding)
            height: Math.max(flick.height, contentHeight + topPadding + bottomPadding)
            wrapMode: ed.wrap ? TextEdit.Wrap : TextEdit.NoWrap
            color: Theme.text
            selectionColor: Theme.tint(0.35)
            selectedTextColor: Theme.text
            font { family: ed.syntax !== "" && ed.syntax !== "Markdown" ? Theme.mono : Theme.font; pointSize: Theme.pt(14) * win.zoom }
            leftPadding: 10
            rightPadding: 28
            topPadding: 14
            bottomPadding: 40
            tabStopDistance: metrics.averageCharacterWidth * 4
            persistentSelection: true
            selectByMouse: true
            focus: true
            onCursorRectangleChanged: flick.ensureVisible(cursorRectangle)
            onTextChanged: {
                if (ed.loading) return;
                if (ed.scratch) autosave.restart();
                else ed.dirty = true;
            }
            // Tab inserts spaces in code, a tab in prose.
            Keys.onTabPressed: e => {
                if (ed.syntax !== "" && ed.syntax !== "Markdown" && ed.syntax !== "Makefile") { area.insert(area.cursorPosition, "    "); e.accepted = true }
                else e.accepted = false;
            }
            Text {
                visible: area.text === "" && ed.scratch
                x: area.leftPadding; y: area.topPadding
                text: "Notes, lists, anything. It saves itself."
                color: Theme.faint
                font: area.font
            }
        }
    }

    Highlight { textEdit: area; syntax: ed.syntax; dark: !Theme.light }
}
