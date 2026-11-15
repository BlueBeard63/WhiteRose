// Island view: which layout the workspace is on now, after SUPER+T or the
// bar's Layout button, and whether that's kept for this workspace (it
// outlasts the login and beats your default) or is your default. Closes by
// itself.
import QtQuick
import qs

Item {
    id: root
    implicitWidth: 360
    implicitHeight: 16 + col.implicitHeight + 14

    readonly property var note: ShellState.layoutNote
    function named(l) { return l ? l.charAt(0).toUpperCase() + l.slice(1) : "" }

    Timer {
        running: true
        interval: 2500
        onTriggered: if (ShellState.islandView === "layoutnote") ShellState.close()
    }

    Column {
        id: col
        x: 22; y: 16
        width: parent.width - 44
        spacing: 2
        Text {
            text: root.note ? "Workspace " + root.note.workspace + ": " + root.named(root.note.layout) : ""
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }
        Text {
            width: parent.width
            wrapMode: Text.Wrap
            text: !root.note ? "" : root.note.kept
                ? "Kept for this workspace. Your default is " + root.named(root.note.byDefault) + "."
                : "Your default layout"
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(11.5) }
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }
    }
}
