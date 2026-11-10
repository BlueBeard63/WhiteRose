// "Show changes" on a clash: what replacing the file that's there with
// the one coming would change, line by line (diff.rs), with the answer
// buttons right here too.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files

Popup {
    id: root
    property string there: ""
    property string coming: ""
    property var diff: null
    // Answered here: replace | newer | keep | skip.
    signal answered(string choice)

    function show(there, coming) {
        root.there = there;
        root.coming = coming;
        root.diff = JSON.parse(Ops.diff(there, coming));
        root.open();
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    width: Math.min(900, (parent?.width ?? 900) - 40)
    height: Math.min(680, (parent?.height ?? 680) - 40)
    modal: true
    padding: 20
    background: Rectangle { color: Theme.surface1; radius: 16; border { width: 1; color: Theme.surface2 } }
    Overlay.modal: Rectangle { color: Qt.rgba(0, 0, 0, 0.35) }

    readonly property color addInk: "#57AB5A"
    readonly property color addBg: Qt.rgba(0.34, 0.67, 0.35, 0.14)
    readonly property color delBg: Qt.rgba(0.9, 0.33, 0.29, 0.14)

    contentItem: Column {
        spacing: 10
        Text {
            width: parent.width
            elide: Text.ElideMiddle
            text: "Replacing “" + root.there.substring(root.there.lastIndexOf("/") + 1) + "”"
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
        }
        Text {
            visible: root.diff?.ok ?? false
            text: !root.diff?.ok ? "" : root.diff.hunks.length === 0 ? "The text is the same (only line endings or nothing at all differ)"
                : root.diff.added + " line" + (root.diff.added === 1 ? "" : "s") + " added, " + root.diff.removed + " removed: red is what's there now, green is what's coming"
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(11.5) }
        }
        Rectangle {
            width: parent.width
            height: root.availableHeight - y - foot.height - 10
            radius: 10
            color: Theme.bg
            border { width: 1; color: Theme.surface2 }
            Text {
                visible: !(root.diff?.ok ?? true)
                anchors.centerIn: parent
                text: root.diff?.error ?? ""
                color: Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(12) }
            }
            ListView {
                id: list
                visible: root.diff?.ok ?? false
                anchors { fill: parent; margins: 6 }
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                ScrollBar.horizontal: ScrollBar { policy: ScrollBar.AsNeeded }
                contentWidth: Math.max(width, longest + 120)
                flickableDirection: Flickable.AutoFlickIfNeeded
                // A hunk, then a gap row ("…") before the next.
                model: root.diff?.ok ? root.diff.hunks.reduce((a, h, i) => a.concat(i > 0 ? [{ kind: "gap" }] : [], h), []) : []
                property real longest: 0
                delegate: Rectangle {
                    id: line
                    required property var modelData
                    width: list.contentWidth
                    height: modelData.kind === "gap" ? 22 : 20
                    color: modelData.kind === "+" ? root.addBg : modelData.kind === "-" ? root.delBg : "transparent"
                    Text {
                        visible: line.modelData.kind === "gap"
                        x: 12; anchors.verticalCenter: parent.verticalCenter
                        text: "⋯"
                        color: Theme.faint
                    }
                    Row {
                        visible: line.modelData.kind !== "gap"
                        anchors.verticalCenter: parent.verticalCenter
                        x: 4
                        spacing: 8
                        Text { width: 36; horizontalAlignment: Text.AlignRight; text: line.modelData.old || ""; color: Theme.faint; font { family: Theme.mono; pointSize: Theme.pt(10) } }
                        Text { width: 36; horizontalAlignment: Text.AlignRight; text: line.modelData.new || ""; color: Theme.faint; font { family: Theme.mono; pointSize: Theme.pt(10) } }
                        Text {
                            width: 12
                            text: line.modelData.kind === " " ? "" : line.modelData.kind
                            color: line.modelData.kind === "+" ? root.addInk : Theme.rose
                            font { family: Theme.mono; pointSize: Theme.pt(10.5); weight: Font.DemiBold }
                        }
                        Text {
                            text: line.modelData.text ?? ""
                            textFormat: Text.PlainText
                            color: Theme.text
                            font { family: Theme.mono; pointSize: Theme.pt(10.5) }
                            Component.onCompleted: if (implicitWidth > list.longest) list.longest = implicitWidth
                        }
                    }
                }
            }
        }
        Item {
            id: foot
            width: parent.width; height: 34
            Row {
                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                spacing: 8
                Button { text: "Skip"; kind: "ghost"; onClicked: { root.answered("skip"); root.close() } }
                Button { text: "Keep both"; onClicked: { root.answered("keep"); root.close() } }
                Button { text: "Replace"; kind: "primary"; onClicked: { root.answered("replace"); root.close() } }
            }
        }
    }
}
