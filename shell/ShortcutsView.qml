// Island view: the keyboard shortcut sheet (SUPER+K; Figma: "Island v2 -
// Shortcuts (SUPER+K)"). The shortcuts worth knowing in three groups, plus
// yours, each on the keys it's on now (Shortcuts.js). Esc, SUPER+K or a
// click elsewhere closes it.
import QtQuick
import qs
import "Shortcuts.js" as Sc

Item {
    id: root
    readonly property var groups: Sc.groups(Compositor.shortcuts)
    readonly property int colWidth: 300
    readonly property int gap: 28
    implicitWidth: 48 + root.groups.length * root.colWidth + Math.max(0, root.groups.length - 1) * root.gap
    implicitHeight: col.implicitHeight + 36

    Component.onCompleted: keys.forceActiveFocus()
    Item { id: keys; focus: true; Keys.onEscapePressed: ShellState.close() }

    component Cap: Rectangle {
        property alias text: capText.text
        width: capText.implicitWidth + 12
        height: capText.implicitHeight + 4
        radius: 5
        color: Theme.surface2
        Text {
            id: capText
            anchors.centerIn: parent
            color: Theme.text
            font { family: Theme.mono; pointSize: Theme.pt(11); weight: Font.Medium }
        }
    }

    Column {
        id: col
        x: 24; y: 18
        width: root.width - 48
        spacing: 18

        Item {
            width: parent.width
            height: title.implicitHeight
            Text {
                id: title
                text: "Keyboard shortcuts"
                color: Theme.text
                font { family: Theme.font; pointSize: Theme.pt(20); weight: Font.DemiBold }
                Accessible.role: Accessible.Heading
                Accessible.name: text
            }
            Row {
                anchors { right: parent.right; verticalCenter: title.verticalCenter }
                spacing: 6
                Cap { text: "ESC" }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "or SUPER+K to close"
                    color: Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(12) }
                }
            }
        }

        Row {
            spacing: root.gap
            Repeater {
                model: root.groups
                delegate: Column {
                    id: group
                    required property var modelData
                    width: root.colWidth
                    spacing: 8
                    Text {
                        text: group.modelData.name
                        color: Theme.muted
                        font { family: Theme.font; pointSize: Theme.pt(11); weight: Font.Medium }
                        Accessible.role: Accessible.Heading
                        Accessible.name: text
                    }
                    Repeater {
                        model: group.modelData.rows
                        delegate: Item {
                            id: row
                            required property var modelData
                            width: group.width
                            height: 20
                            Accessible.role: Accessible.StaticText
                            Accessible.name: row.modelData.label
                            Accessible.description: row.modelData.keys.join(" + ")
                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                width: parent.width - caps.width - 8
                                elide: Text.ElideRight
                                text: row.modelData.label
                                color: Theme.text
                                font { family: Theme.font; pointSize: Theme.pt(13) }
                            }
                            Row {
                                id: caps
                                anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                                spacing: 3
                                Repeater {
                                    model: row.modelData.keys
                                    delegate: Cap { required property string modelData; text: modelData }
                                }
                            }
                        }
                    }
                }
            }
        }

        Text {
            text: "Change them in Settings › Keyboard. Your own shortcuts show here too."
            color: Theme.faint
            font { family: Theme.font; pointSize: Theme.pt(12) }
        }
    }
}
