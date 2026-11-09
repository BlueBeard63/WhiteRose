// Where you are: breadcrumbs to click, or (Ctrl+L, or a click on the empty
// part) a path to type, with folders suggested as you go. Tab takes the
// first suggestion, Enter goes, Esc stops typing.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files

Rectangle {
    id: bar
    property string path
    property bool hidden: false
    property bool editing: false
    property var suggestions: []
    property int pick: -1
    signal go(string path)
    height: 34
    radius: 10
    color: editing ? Theme.bg : Theme.surface1
    border { width: editing ? 1 : 0; color: Theme.muted }

    function edit() {
        bar.editing = true;
        input.text = bar.path === "/" ? "/" : bar.path + "/";
        input.forceActiveFocus();
        input.selectAll();
    }
    signal done()               // Esc: back to the files
    function stop() { bar.editing = false; bar.suggestions = []; bar.pick = -1 }
    function submit() {
        const typed = bar.pick >= 0 ? bar.suggestions[bar.pick] : input.text;
        const p = Fs.resolve(typed);
        if (p === "") { shake.restart(); return }
        bar.stop();
        bar.go(p);
    }

    // ---- breadcrumbs ---------------------------------------------------------------
    MouseArea { anchors.fill: parent; visible: !bar.editing; cursorShape: Qt.IBeamCursor; onClicked: bar.edit() }
    // Too long for the bar: "… > Folder > Folder", as many of the last
    // folders as fit. "…" (like any empty part of the bar) shows the whole
    // path as text, selected, to copy or change.
    readonly property var chain: JSON.parse(Fs.crumbs(bar.path))
    readonly property int skip: {
        const room = bar.width - 12, chev = 11, pad = 16;
        const w = bar.chain.map(c => metrics.advanceWidth(c.name) + pad);
        const all = w.reduce((a, x) => a + x, 0) + chev * (w.length - 1);
        if (all <= room) return 0;
        // Keep the last folder whatever happens; add earlier ones while they fit.
        let used = metrics.advanceWidth("…") + pad + chev + w[w.length - 1];
        let keep = 1;
        while (keep < w.length - 1 && used + w[w.length - 1 - keep] + chev <= room) { used += w[w.length - 1 - keep] + chev; keep++ }
        return w.length - keep;
    }
    readonly property var shown: bar.skip > 0
        ? [{ name: "…", path: "", more: true }].concat(bar.chain.slice(bar.skip))
        : bar.chain
    FontMetrics { id: metrics; font { family: Theme.font; pointSize: Theme.pt(12.5); weight: Font.Medium } }

    Row {
        visible: !bar.editing
        anchors { left: parent.left; leftMargin: 6; verticalCenter: parent.verticalCenter }
        spacing: 0
        Repeater {
            model: bar.shown
            delegate: Row {
                required property var modelData
                required property int index
                anchors.verticalCenter: parent.verticalCenter
                Icon { visible: index > 0; anchors.verticalCenter: parent.verticalCenter; name: "chevR"; size: 11; color: Theme.faint }
                Rectangle {
                    width: crumbText.implicitWidth + 16; height: 26; radius: 7
                    anchors.verticalCenter: parent.verticalCenter
                    color: crumbArea.containsMouse ? Theme.surface2 : "transparent"
                    Text {
                        id: crumbText
                        anchors.centerIn: parent
                        text: modelData.name
                        elide: Text.ElideMiddle
                        width: Math.min(implicitWidth, bar.width - 60)
                        color: modelData.path === bar.path ? Theme.text : Theme.muted
                        font { family: Theme.font; pointSize: Theme.pt(12.5); weight: modelData.path === bar.path ? Font.Medium : Font.Normal }
                    }
                    MouseArea {
                        id: crumbArea
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: modelData.more ? bar.edit() : bar.go(modelData.path)
                    }
                }
            }
        }
    }

    // ---- typing a path -------------------------------------------------------------
    TextInput {
        id: input
        visible: bar.editing
        anchors { fill: parent; leftMargin: 12; rightMargin: 12 }
        verticalAlignment: TextInput.AlignVCenter
        color: Theme.text
        selectionColor: Theme.tint(0.4)
        selectedTextColor: Theme.text
        clip: true
        font { family: Theme.mono; pointSize: Theme.pt(12.5) }
        onTextEdited: { bar.suggestions = JSON.parse(Fs.suggest(text, bar.hidden)); bar.pick = -1 }
        onActiveFocusChanged: if (!activeFocus && bar.editing) bar.stop()
        Keys.onReturnPressed: bar.submit()
        Keys.onEnterPressed: bar.submit()
        Keys.onEscapePressed: { bar.stop(); bar.done() }
        Keys.onDownPressed: bar.pick = Math.min(bar.suggestions.length - 1, bar.pick + 1)
        Keys.onUpPressed: bar.pick = Math.max(-1, bar.pick - 1)
        Keys.onTabPressed: {
            const s = bar.suggestions[Math.max(0, bar.pick)];
            if (s) { input.text = s + "/"; bar.suggestions = JSON.parse(Fs.suggest(input.text, bar.hidden)); bar.pick = -1 }
        }
    }
    SequentialAnimation {
        id: shake
        NumberAnimation { target: input; property: "anchors.leftMargin"; to: 6; duration: 50 }
        NumberAnimation { target: input; property: "anchors.leftMargin"; to: 18; duration: 70 }
        NumberAnimation { target: input; property: "anchors.leftMargin"; to: 12; duration: 50 }
    }

    // Suggestions under the bar.
    Rectangle {
        visible: bar.editing && bar.suggestions.length > 0
        anchors { left: parent.left; right: parent.right; top: parent.bottom; topMargin: 4 }
        height: sugg.implicitHeight + 8
        radius: 10
        z: 10
        color: Theme.surface1
        border { width: 1; color: Theme.surface2 }
        Column {
            id: sugg
            x: 4; y: 4
            width: parent.width - 8
            Repeater {
                model: bar.suggestions
                delegate: Rectangle {
                    required property string modelData
                    required property int index
                    width: sugg.width; height: 28; radius: 7
                    color: index === bar.pick || sArea.containsMouse ? Theme.surface2 : "transparent"
                    Text {
                        anchors { left: parent.left; leftMargin: 10; verticalCenter: parent.verticalCenter }
                        text: modelData.replace(Fs.home(), "~")
                        color: Theme.text
                        font { family: Theme.mono; pointSize: Theme.pt(12) }
                    }
                    MouseArea { id: sArea; anchors.fill: parent; hoverEnabled: true; onPressed: { bar.pick = index; bar.submit() } }
                }
            }
        }
    }
}
