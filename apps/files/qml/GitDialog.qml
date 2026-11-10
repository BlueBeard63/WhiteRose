// Git, where words are needed: a commit's message, the address to clone,
// a new branch's name (gitops.rs does the work, with your git, its config,
// credential helper and SSH keys). Cloning shows how far it's got.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files

Popup {
    id: root
    // commit | clone | branch
    property string mode: "commit"
    property string dir: ""
    // Commit: how many changes are staged (0: offer to commit every change).
    property int staged: 0
    property int job: -1
    property bool busy: false
    property string error: ""
    property real percent: -1
    property string what: ""
    signal done(string mode, var result)

    function show(mode, dir, staged) {
        root.mode = mode; root.dir = dir; root.staged = staged ?? 0;
        root.error = ""; root.busy = false; root.percent = -1; root.what = "";
        text.text = "";
        all.checked = root.staged === 0;
        root.open();
        text.focusField();
    }
    function go() {
        root.error = "";
        root.busy = true;
        const t = text.text;
        const what = root.mode === "commit" ? (all.checked ? "commit-all" : "commit") : root.mode;
        root.job = Ops.gitDo(root.dir, what, t, "[]");
    }
    Connections {
        target: Ops
        function onGitDone(id, result) {
            if (id !== root.job) return;
            const r = JSON.parse(result);
            if (!r.done) { root.percent = r.percent; root.what = r.what; return }
            root.busy = false;
            if (!r.ok) { root.error = r.error; return }
            root.close();
            root.done(root.mode, r);
        }
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    width: 480
    modal: true
    padding: 20
    background: Rectangle { color: Theme.surface1; radius: 16; border { width: 1; color: Theme.surface2 } }
    Overlay.modal: Rectangle { color: Qt.rgba(0, 0, 0, 0.35) }
    closePolicy: root.busy ? Popup.NoAutoClose : (Popup.CloseOnEscape | Popup.CloseOnPressOutside)

    contentItem: Column {
        spacing: 12
        Text {
            text: ({ commit: "Commit", clone: "Clone a repository here", branch: "New branch" })[root.mode]
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
        }
        Field {
            id: text
            width: parent.width
            mono: root.mode !== "commit"
            placeholder: ({ commit: "What it does, e.g. “Fix the date in the footer”", clone: "https://github.com/… or git@github.com:…", branch: "e.g. fix/footer-date" })[root.mode]
            onAccepted: if (!root.busy && text.text.trim() !== "") root.go()
            onTextChanged: root.error = ""
        }
        Row {
            visible: root.mode === "commit"
            spacing: 6
            Toggle { id: all; anchors.verticalCenter: parent.verticalCenter }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: root.staged > 0 ? "Every changed file, not just the " + root.staged + " staged" : "Every changed file (nothing's staged)"
                color: Theme.text
                font { family: Theme.font; pointSize: Theme.pt(11.5) }
            }
        }
        Text {
            visible: root.mode === "branch"
            width: parent.width
            wrapMode: Text.Wrap
            text: "Made from where you are now, and switched to. Your changes come with it."
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(11) }
        }
        // Cloning: how far.
        Column {
            visible: root.busy && root.mode === "clone"
            width: parent.width
            spacing: 6
            Text { text: root.what ? root.what + (root.percent >= 0 ? " · " + root.percent + "%" : "") : "Connecting…"; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11) } }
            Rectangle {
                width: parent.width; height: 4; radius: 2; color: Theme.surface2
                Rectangle { width: parent.width * Math.max(0, root.percent) / 100; height: 4; radius: 2; color: Theme.rose }
            }
        }
        Text {
            visible: root.error !== ""
            width: parent.width
            wrapMode: Text.Wrap
            text: root.error
            color: Theme.rose
            font { family: Theme.font; pointSize: Theme.pt(11.5) }
        }
        Item {
            width: parent.width; height: 34
            Row {
                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                spacing: 8
                Button {
                    text: "Cancel"
                    onClicked: { if (root.busy) { Ops.stopCleanup(root.job); root.busy = false; return } root.close() }
                }
                Button {
                    text: ({ commit: "Commit", clone: "Clone", branch: "Make and switch" })[root.mode]
                    kind: "primary"
                    enabled: !root.busy && text.text.trim() !== ""
                    onClicked: root.go()
                }
            }
        }
    }
}
