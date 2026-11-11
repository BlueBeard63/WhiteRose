// Opening something that would run code: say so first (in a strip above
// the files). Somewhere files from
// outside land, running it takes ticking "I trust this file"; launchers
// from there don't run at all. "Show contents" opens it as text instead.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files

Strip {
    id: root
    icon: root.risky ? "alert" : "terminal"
    tone: root.risky ? "danger" : "plain"
    focusOn: root.risky ? null : runButton
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

    Column {
        anchors.verticalCenter: parent.verticalCenter
        width: root.room
        spacing: 2
        Text {
            width: parent.width
            elide: Text.ElideMiddle
            text: "“" + (root.entry?.name ?? "") + "” is " + (root.kind === "script" ? "a script" : root.kind === "launcher" ? "a launcher" : "a program")
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(12); weight: Font.Medium }
        }
        Text {
            width: parent.width
            wrapMode: Text.Wrap
            text: root.kind === "launcher"
                ? (root.risky ? root.why + " Launchers from there don't run from Files: they can start any command." : "Launchers run from the app list, not from Files.")
                : (root.risky ? root.why + " Only run it if you trust where it came from." : "Running it lets it do anything you can.")
            color: root.risky ? Theme.text : Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(10.5) }
        }
    }
    buttons: [
        Row {
            visible: root.risky && root.kind !== "launcher"
            anchors.verticalCenter: parent.verticalCenter
            spacing: 6
            Toggle { anchors.verticalCenter: parent.verticalCenter; checked: root.trusted; onToggled: on => root.trusted = on }
            Text { anchors.verticalCenter: parent.verticalCenter; text: "I trust it"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(11) } }
            Item { width: 6; height: 1 }
        },
        Button { text: "Cancel"; kind: "ghost"; onClicked: root.close() },
        Button { text: "Show contents"; onClicked: { const e = root.entry; root.close(); root.showContents(e) } },
        Button {
            id: runButton
            visible: root.kind !== "launcher"
            text: "Run"
            kind: root.risky ? "danger" : "primary"
            enabled: root.canRun
            Keys.onReturnPressed: e => { e.accepted = true; Qt.callLater(() => runButton.clicked()) }
            onClicked: { if (!root.canRun) return; const e = root.entry; root.close(); root.run(e) }
        }
    ]
}
