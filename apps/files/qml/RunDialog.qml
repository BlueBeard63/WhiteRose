// Opening something that would run code: say so first. Somewhere files from
// outside land, running it takes ticking "I trust this file"; launchers
// from there don't run at all. "Show contents" opens it as text instead.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files

Popup {
    id: root
    property var entry: null
    // program | script | launcher
    property string kind: "program"
    property string why: ""
    property bool trusted: false
    readonly property bool risky: root.why !== ""
    readonly property bool canRun: root.kind !== "launcher" && (!root.risky || root.trusted)
    signal run(var entry)
    signal showContents(var entry)

    function ask(entry, info) {
        root.entry = entry;
        root.kind = info.kind;
        root.why = info.why;
        root.trusted = false;
        root.open();
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    width: 420
    modal: true
    focus: true
    padding: 22
    background: Rectangle { color: Theme.surface1; radius: 16; border { width: 1; color: Theme.surface2 } }
    Overlay.modal: Rectangle { color: Qt.rgba(0, 0, 0, 0.4) }

    contentItem: Column {
        spacing: 12
        Row {
            spacing: 12
            Rectangle {
                width: 38; height: 38; radius: 11
                color: root.risky ? Qt.rgba(Theme.rose.r, Theme.rose.g, Theme.rose.b, 0.18) : Theme.surface2
                Icon { anchors.centerIn: parent; name: root.risky ? "alert" : "terminal"; size: 18; color: root.risky ? Theme.rose : Theme.text }
            }
            Column {
                anchors.verticalCenter: parent.verticalCenter
                width: 330
                Text {
                    width: parent.width
                    text: "“" + (root.entry?.name ?? "") + "” is " + (root.kind === "script" ? "a script" : root.kind === "launcher" ? "a launcher" : "a program")
                    wrapMode: Text.Wrap
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
                }
            }
        }
        Text {
            width: 376
            wrapMode: Text.Wrap
            text: root.kind === "launcher"
                ? (root.risky ? root.why + " Launchers from there don't run from Files: they can start any command." : "Launchers start a command; they run from the app list rather than from Files.")
                : (root.risky ? root.why + " Only run it if you trust where it came from: it can do anything you can." : "Running it lets it do anything you can.")
            color: root.risky ? Theme.rose : Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(12) }
        }
        Row {
            visible: root.risky && root.kind !== "launcher"
            spacing: 8
            Toggle { checked: root.trusted; onToggled: on => root.trusted = on }
            Text { anchors.verticalCenter: parent.verticalCenter; text: "I trust this file"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(12) } }
        }
        Row {
            anchors.right: parent.right
            spacing: 8
            Button { text: "Cancel"; kind: "ghost"; onClicked: root.close() }
            Button {
                text: "Show contents"
                onClicked: { const e = root.entry; root.close(); root.showContents(e) }
            }
            Button {
                visible: root.kind !== "launcher"
                text: "Run"
                kind: root.risky ? "danger" : "primary"
                enabled: root.canRun
                onClicked: { if (!root.canRun) return; const e = root.entry; root.close(); root.run(e) }
            }
        }
    }
}
