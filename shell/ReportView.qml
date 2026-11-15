// Island view: report a problem, one step at a time (docs/plans/bug-reports.md).
//   1 Which problem   (a crash, a snapshot, or one taken now)
//   2 What kind       (crashed, froze, looks wrong, doesn't work, slow)
//   3 Where           (and the title made from it)
//   4 What happened   (the only thing to write)
//   5 What's sent     (each section, switch off what you'd rather not send)
// Everything is filled in from the snapshot (Report.js's guesses) and can be
// changed. The draft is kept by whiterosed, so closing the island (Not now,
// Esc) or the shell restarting keeps it. Open report saves the report file
// in Downloads, opens the prefilled bug form (a verified build only) and
// the folder with the file in Files, to drag it into the issue.
import QtQuick
import Quickshell
import qs
import qs.settings
import "Report.js" as Rep

Item {
    id: root
    implicitWidth: 600
    implicitHeight: body.implicitHeight + 40

    property var draft: null
    property var list: []          // report_snapshots
    property var rec: null         // the chosen record
    property var snap: null        // its snapshot
    property string recTitle: ""
    property var preview: []       // step 5's sections
    property var build: null       // report_build
    property bool busy: false
    property string error: ""
    property var done: null        // report_open's answer
    property real now: Date.now()
    readonly property int steps: 5
    readonly property int step: draft?.step ?? 1

    function update(patch) {
        draft = Object.assign({}, draft, patch);
        Daemon.request({ op: "report_draft_save", value: draft }, () => {});
    }

    function load() {
        Daemon.request({ op: "report_snapshots" }, v => {
            if (!root) return;
            root.list = v ?? [];
            Daemon.request({ op: "report_draft" }, d => {
                if (!root) return;
                const from = ShellState.reportFrom;
                ShellState.reportFrom = "";
                // Opened from a crash: that one, whatever was left half done.
                if (d && (!from || d.snapshot === from)) root.draft = Object.assign({ off: [], edited: false }, d);
                else root.draft = Rep.newDraft(root.list, from, Date.now());
                root.loadSnapshot();
            });
        });
        Daemon.request({ op: "report_build" }, v => { if (root) root.build = v; });
    }
    Component.onCompleted: load()

    function loadSnapshot() {
        if (!draft.snapshot) { rec = null; snap = null; recTitle = ""; return; }
        Daemon.request({ op: "report_snapshot", name: draft.snapshot }, (v, err) => {
            if (!root) return;
            if (err) { root.error = err; root.update({ snapshot: "", step: 1 }); return; }
            root.rec = v.record;
            root.snap = v.snapshot;
            root.recTitle = v.title;
            root.update(Rep.fill(root.draft, v.record, v.snapshot, v.title));
        });
    }

    function choose(id) {
        // Another problem: the guesses start again (a title you wrote stays).
        update({ snapshot: id, kind: "", where: "" });
        loadSnapshot();
    }

    // Hiding a step hides its text field; if that held the keyboard, the
    // island would take it as losing focus and close. So the view takes it
    // back first.
    function keepFocus() { root.forceActiveFocus() }
    onStepChanged: if (step === 4) Qt.callLater(() => whatEdit.forceActiveFocus())

    function next() {
        keepFocus();
        error = "";
        if (!Rep.canGoOn(draft)) return;
        if (step === 1 && !draft.snapshot) {
            // "Something that's happening now": a snapshot first.
            busy = true;
            Daemon.request({ op: "report_snapshot_now" }, (v, err) => {
                if (!root) return;
                root.busy = false;
                if (err) { root.error = err; return; }
                root.update({ snapshot: v.id, step: 2, kind: "", where: "" });
                root.loadSnapshot();
            });
            return;
        }
        if (step === 4) {
            busy = true;
            Daemon.request({ op: "report_preview", value: draft }, (v, err) => {
                if (!root) return;
                root.busy = false;
                if (err) { root.error = err; return; }
                root.preview = v ?? [];
                root.update({ step: 5 });
            });
            return;
        }
        if (step === 5) { send(); return; }
        update({ step: step + 1 });
    }
    function back() { keepFocus(); error = ""; if (step > 1) update({ step: step - 1 }); }

    function send() {
        keepFocus();
        busy = true;
        Daemon.request({ op: "report_open", value: draft }, (v, err) => {
            if (!root) return;
            root.busy = false;
            if (err) { root.error = err; return; }
            root.done = v;
            if (v.url) Compositor.launch("", ["xdg-open", v.url]);
            Compositor.launch("", ["whiterose-files", v.file.substring(0, v.file.lastIndexOf("/"))]);
        });
    }
    function toggleSection(key) {
        const off = (draft.off ?? []).slice();
        const i = off.indexOf(key);
        if (i >= 0) off.splice(i, 1); else off.push(key);
        update({ off: off });
    }

    readonly property bool verified: build?.state === "verified"
    readonly property string buildNote: {
        const b = build;
        if (!b) return "Checking this build…";
        if (b.state === "verified") return "Goes to " + b.repo + " as a bug report. Drag the saved file into it.";
        const why = {
            modified: "This build doesn't match a WhiteRose release, so it can't report to " + (b.repo || "the WhiteRose repository") + ". Report it to whoever made the build.",
            unlisted: "This version can't be checked against its release (" + (b.reason || "released before builds could be checked") + ").",
            development: "This is a build being worked on, not an installed release.",
            unknown: "Couldn't check this build (" + (b.reason || "offline") + ").",
        }[b.state] ?? b.reason;
        return why + " The report is saved to a file only.";
    }

    Column {
        id: body
        x: 24; y: 20
        width: parent.width - 48
        spacing: 16

        // ---- header ----------------------------------------------------------------
        Item {
            width: parent.width; height: 42
            Rectangle {
                id: badge
                width: 42; height: 42; radius: 12
                color: Theme.tint(0.14)
                Icon { anchors.centerIn: parent; name: root.done ? "check" : "alert"; size: 19; color: Theme.rose }
            }
            Column {
                anchors { left: badge.right; leftMargin: 14; verticalCenter: parent.verticalCenter; right: stepCount.left; rightMargin: 12 }
                spacing: 2
                Text {
                    text: root.done ? (root.done.url ? "Report opened in your browser" : "Report saved") : "Report a problem"
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(14.5); weight: Font.DemiBold }
                }
                Text {
                    width: parent.width
                    elide: Text.ElideRight
                    visible: !root.done
                    text: ["Which problem is this about?", "What kind of problem is it?", "Where did it happen?", "What happened?", "What's sent"][root.step - 1]
                    color: Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(11.5) }
                }
            }
            Text {
                id: stepCount
                visible: !root.done
                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                text: root.step + " of " + root.steps
                color: Theme.faint
                font { family: Theme.mono; pointSize: Theme.pt(10.5) }
            }
        }

        // ---- 1: which problem ---------------------------------------------------------
        Column {
            visible: !root.done && root.step === 1
            width: parent.width
            spacing: 2
            Repeater {
                model: [{ id: "", title: "Something that's happening now", time: 0 }].concat(root.list.slice(0, 5))
                delegate: Rectangle {
                    id: pickRow
                    required property var modelData
                    readonly property bool chosen: (root.draft?.snapshot ?? "") === modelData.id
                    width: parent.width; height: 40; radius: 10
                    color: chosen ? Theme.surface2 : pickMouse.containsMouse ? Theme.surface1 : "transparent"
                    Accessible.role: Accessible.RadioButton
                    Accessible.name: label.text
                    Accessible.checked: chosen
                    Accessible.onPressAction: root.choose(modelData.id)
                    Icon {
                        id: pickIcon
                        x: 12; anchors.verticalCenter: parent.verticalCenter
                        name: pickRow.modelData.id === "" ? "camera" : pickRow.modelData.what === "manual" ? "camera" : "alert"
                        size: 14; color: pickRow.chosen ? Theme.rose : Theme.muted
                    }
                    Text {
                        id: label
                        anchors { left: pickIcon.right; leftMargin: 10; right: tick.left; rightMargin: 8; verticalCenter: parent.verticalCenter }
                        elide: Text.ElideRight
                        text: pickRow.modelData.id === "" ? pickRow.modelData.title : Rep.snapshotLabel(pickRow.modelData, root.now)
                        color: Theme.text
                        font { family: Theme.font; pointSize: Theme.pt(12.5) }
                    }
                    Icon { id: tick; visible: pickRow.chosen; anchors { right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter } name: "check"; size: 14; color: Theme.rose }
                    MouseArea { id: pickMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: root.choose(pickRow.modelData.id) }
                }
            }
            Text {
                width: parent.width
                topPadding: 6
                wrapMode: Text.Wrap
                text: "Crashes are recorded on their own. For anything else, SUPER+SHIFT+B records the moment it happens."
                color: Theme.faint
                font { family: Theme.font; pointSize: Theme.pt(11) }
            }
        }

        // ---- 2: what kind -------------------------------------------------------------
        Column {
            visible: !root.done && root.step === 2
            width: parent.width
            spacing: 10
            Segmented {
                anchors.verticalCenter: undefined
                options: Rep.kinds.map(k => ({ label: k.label, value: k.value }))
                value: root.draft?.kind ?? ""
                onPicked: v => root.update(Rep.fill(Object.assign({}, root.draft, { kind: v }), root.rec, root.snap, root.recTitle))
            }
            Text {
                width: parent.width
                wrapMode: Text.Wrap
                text: "Picked from what was recorded. Change it if it's not right."
                color: Theme.faint
                font { family: Theme.font; pointSize: Theme.pt(11) }
            }
        }

        // ---- 3: where, and the title ----------------------------------------------------
        Column {
            visible: !root.done && root.step === 3
            width: parent.width
            spacing: 10
            // A dropdown that opens in place (the island grows with it).
            Rectangle {
                id: whereBox
                property bool open: false
                width: parent.width; height: 36; radius: 10
                color: Theme.bg
                border { width: 1; color: open ? Theme.muted : Theme.surface2 }
                Accessible.role: Accessible.ComboBox
                Accessible.name: "Where"
                Accessible.onPressAction: open = !open
                Text {
                    anchors { left: parent.left; leftMargin: 12; verticalCenter: parent.verticalCenter }
                    text: root.draft?.where ?? ""
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(12.5) }
                }
                Icon { anchors { right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter } name: whereBox.open ? "chevUp" : "chevDown"; size: 14; color: Theme.muted }
                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: whereBox.open = !whereBox.open }
            }
            Rectangle {
                visible: whereBox.open
                width: parent.width
                height: Math.min(areaList.contentHeight + 8, 214)
                radius: 10
                color: Theme.surface1
                ListView {
                    id: areaList
                    anchors { fill: parent; margins: 4 }
                    clip: true
                    model: Rep.areaOptions(root.snap, root.draft?.where)
                    delegate: Rectangle {
                        id: areaRow
                        required property string modelData
                        width: areaList.width; height: 30; radius: 7
                        color: areaMouse.containsMouse ? Theme.surface2 : "transparent"
                        Text {
                            anchors { left: parent.left; leftMargin: 10; verticalCenter: parent.verticalCenter }
                            text: areaRow.modelData
                            color: areaRow.modelData === root.draft?.where ? Theme.rose : Theme.text
                            font { family: Theme.font; pointSize: Theme.pt(12) }
                        }
                        MouseArea {
                            id: areaMouse
                            anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor
                            onClicked: {
                                whereBox.open = false;
                                root.update(Rep.fill(Object.assign({}, root.draft, { where: areaRow.modelData }), root.rec, root.snap, root.recTitle));
                            }
                        }
                    }
                }
            }
            Text {
                text: "Title"
                topPadding: 4
                color: Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(11.5) }
            }
            Field {
                id: titleField
                width: parent.width
                placeholder: "A short line saying what went wrong"
                label: "Title"
                text: root.draft?.title ?? ""
                onTextChanged: if (root.draft && text !== root.draft.title) root.update({ title: text, edited: true })
            }
        }

        // ---- 4: what happened -----------------------------------------------------------
        Column {
            visible: !root.done && root.step === 4
            width: parent.width
            spacing: 8
            Rectangle {
                width: parent.width; height: 120; radius: 10
                color: Theme.bg
                border { width: 1; color: whatEdit.activeFocus ? Theme.muted : Theme.surface2 }
                Flickable {
                    id: whatFlick
                    anchors { fill: parent; margins: 12 }
                    contentHeight: whatEdit.implicitHeight
                    clip: true
                    TextEdit {
                        id: whatEdit
                        width: whatFlick.width
                        wrapMode: TextEdit.Wrap
                        color: Theme.text
                        selectionColor: Theme.rose
                        selectedTextColor: Theme.bg
                        font { family: Theme.font; pointSize: Theme.pt(12.5) }
                        text: root.draft?.what ?? ""
                        Accessible.role: Accessible.EditableText
                        Accessible.name: "What happened"
                        onTextChanged: if (root.draft && text !== root.draft.what) root.update({ what: text })
                        onCursorRectangleChanged: {
                            if (cursorRectangle.y + cursorRectangle.height > whatFlick.contentY + whatFlick.height)
                                whatFlick.contentY = cursorRectangle.y + cursorRectangle.height - whatFlick.height;
                        }
                        Text {
                            visible: whatEdit.text === ""
                            width: parent.width
                            wrapMode: Text.Wrap
                            text: "What were you doing, what did you expect, and what happened instead?"
                            color: Theme.faint
                            font: whatEdit.font
                        }
                    }
                }
                MouseArea { anchors.fill: parent; cursorShape: Qt.IBeamCursor; onClicked: whatEdit.forceActiveFocus(); propagateComposedEvents: true }
            }
            Text {
                width: parent.width
                wrapMode: Text.Wrap
                text: "Steps to make it happen again help most."
                color: Theme.faint
                font { family: Theme.font; pointSize: Theme.pt(11) }
            }
        }

        // ---- 5: what's sent ---------------------------------------------------------------
        Column {
            visible: !root.done && root.step === 5
            width: parent.width
            spacing: 8
            Text {
                width: parent.width
                wrapMode: Text.Wrap
                text: root.buildNote
                color: root.verified ? Theme.muted : Theme.rose
                font { family: Theme.font; pointSize: Theme.pt(11.5) }
            }
            Rectangle {
                width: parent.width
                height: sectionCol.implicitHeight
                radius: 12
                color: Theme.surface1
                clip: true
                Column {
                    id: sectionCol
                    width: parent.width
                    Repeater {
                        model: root.preview
                        delegate: Column {
                            id: sec
                            required property var modelData
                            required property int index
                            property bool opened: false
                            readonly property bool on: !(root.draft?.off ?? []).includes(modelData.key)
                            width: sectionCol.width
                            Item {
                                width: parent.width; height: 40
                                Rectangle { visible: sec.index > 0; width: parent.width; height: 1; color: Theme.surface2 }
                                Icon {
                                    id: secChev
                                    x: 12; anchors.verticalCenter: parent.verticalCenter
                                    name: sec.opened ? "chevDown" : "chevR"; size: 12; color: Theme.muted
                                }
                                Text {
                                    anchors { left: secChev.right; leftMargin: 8; verticalCenter: parent.verticalCenter }
                                    text: ({ whiterose: "WhiteRose", system: "System and hardware", load: "Load at the time", windows: "Open windows (app names only)", output: "What it printed last", log: "Session log (last lines)" })[sec.modelData.key] ?? sec.modelData.key
                                    color: sec.on ? Theme.text : Theme.faint
                                    font { family: Theme.font; pointSize: Theme.pt(12) }
                                }
                                MouseArea { anchors { fill: parent; rightMargin: 64 } cursorShape: Qt.PointingHandCursor; onClicked: sec.opened = !sec.opened }
                                Toggle {
                                    anchors { right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter }
                                    label: "Include " + sec.modelData.key
                                    checked: sec.on
                                    onToggled: root.toggleSection(sec.modelData.key)
                                }
                            }
                            Rectangle {
                                visible: sec.opened
                                x: 12; width: parent.width - 24
                                height: Math.min(secText.implicitHeight + 16, 150)
                                radius: 8
                                color: Theme.bg
                                Flickable {
                                    anchors { fill: parent; margins: 8 }
                                    contentHeight: secText.implicitHeight
                                    clip: true
                                    Text {
                                        id: secText
                                        width: parent.width
                                        wrapMode: Text.WrapAnywhere
                                        text: sec.modelData.text
                                        color: sec.on ? Theme.muted : Theme.faint
                                        font { family: Theme.mono; pointSize: Theme.pt(9.5) }
                                    }
                                }
                            }
                            Item { visible: sec.opened; width: 1; height: 10 }
                        }
                    }
                }
            }
        }

        // ---- done ---------------------------------------------------------------------------
        Text {
            visible: !!root.done
            width: parent.width
            wrapMode: Text.Wrap
            text: root.done
                ? (root.done.url
                    ? "Drag " + root.done.file.substring(root.done.file.lastIndexOf("/") + 1) + " from Files into the issue, check it reads right, and submit it."
                    : "Saved as " + root.done.file + ". " + (root.build?.state === "modified" ? "Send it to whoever made this build." : "Attach it wherever you report the problem."))
                : ""
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(11.5) }
        }

        Text {
            visible: root.error !== ""
            width: parent.width
            wrapMode: Text.Wrap
            text: root.error
            color: Theme.rose
            font { family: Theme.font; pointSize: Theme.pt(11.5) }
        }

        // ---- buttons ----------------------------------------------------------------------
        Item {
            width: parent.width; height: 32
            Button {
                anchors.left: parent.left
                visible: !root.done
                text: "Not now"; kind: "ghost"
                onClicked: ShellState.close()
            }
            Row {
                anchors.right: parent.right
                spacing: 8
                Button {
                    visible: !root.done && root.step > 1
                    text: "Back"
                    enabled: !root.busy
                    onClicked: root.back()
                }
                Button {
                    visible: !root.done
                    kind: "primary"
                    enabled: !root.busy && root.draft !== null && Rep.canGoOn(root.draft)
                    text: root.busy ? "Wait…" : root.step < 5 ? "Next" : root.verified ? "Open report" : "Save report"
                    onClicked: root.next()
                }
                Button {
                    visible: !!root.done
                    text: "Close"; kind: "primary"
                    onClicked: ShellState.close()
                }
            }
        }
    }
}
