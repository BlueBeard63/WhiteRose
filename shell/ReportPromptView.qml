// Island view: "Report this" after a crash (the shell or daemon died and
// started again, or the compositor took the last session down), or
// "Snapshot saved" after SUPER+SHIFT+B. Report opens the report flow on
// that snapshot, at step 2; Not now (Later) closes it. A crash is offered
// until one of the two is pressed (ShellState.answerPrompt); Settings ›
// About can still report it after. A snapshot's prompt goes by itself
// after 8 s if left alone.
import QtQuick
import qs
import qs.settings
import "Report.js" as Rep

Item {
    id: root
    implicitWidth: 520
    implicitHeight: body.implicitHeight + 36

    readonly property var s: ShellState.reportPrompt
    readonly property var text: Rep.promptText(s, Date.now())
    readonly property bool snapshot: s?.what === "manual"

    Timer {
        running: root.snapshot && !hover.hovered
        interval: 8000
        onTriggered: ShellState.close()
    }
    HoverHandler { id: hover }

    Row {
        id: body
        x: 22; y: 18
        width: parent.width - 44
        spacing: 14
        Rectangle {
            width: 40; height: 40; radius: 12
            color: Theme.tint(0.14)
            Icon { anchors.centerIn: parent; name: root.snapshot ? "camera" : "alert"; size: 18; color: Theme.rose }
        }
        Column {
            width: parent.width - 54
            spacing: 12
            Column {
                width: parent.width
                spacing: 3
                Text {
                    width: parent.width
                    elide: Text.ElideRight
                    text: root.text[0]
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(13.5); weight: Font.DemiBold }
                }
                Text {
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: root.text[1]
                    color: Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(11.5) }
                }
            }
            Row {
                spacing: 8
                Button {
                    text: "Report"
                    kind: "primary"
                    onClicked: ShellState.answerPrompt(true)
                }
                Button {
                    text: root.snapshot ? "Later" : "Not now"
                    kind: "ghost"
                    onClicked: ShellState.answerPrompt(false)
                }
            }
        }
    }
}
