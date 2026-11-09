// A dropdown menu in the desktop's style: `items` is [{ label, shortcut?,
// danger?, enabled?, separator?, run }]. Open it with popup(x, y) or open().
import QtQuick
import QtQuick.Controls

Popup {
    id: menu
    property var items: []
    padding: 6
    width: Math.max(200, list.implicitWidth + 12)
    background: Rectangle { color: Theme.surface1; radius: 12; border { width: 1; color: Theme.surface2 } }
    contentItem: Column {
        id: list
        spacing: 2
        Repeater {
            model: menu.items
            delegate: Item {
                required property var modelData
                width: Math.max(menu.width - 12, row.implicitWidth + 24)
                height: modelData.separator ? 9 : 30
                Rectangle { visible: !!modelData.separator; anchors.centerIn: parent; width: parent.width - 8; height: 1; color: Theme.surface2 }
                Rectangle {
                    visible: !modelData.separator
                    anchors.fill: parent
                    radius: 8
                    color: area.containsMouse && modelData.enabled !== false ? Theme.surface2 : "transparent"
                    Row {
                        id: row
                        anchors { left: parent.left; leftMargin: 12; right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter }
                        Text {
                            width: parent.width - keys.width
                            text: modelData.label ?? ""
                            color: modelData.enabled === false ? Theme.faint : modelData.danger ? Theme.rose : Theme.text
                            font { family: Theme.font; pointSize: Theme.pt(12.5) }
                        }
                        Text {
                            id: keys
                            text: modelData.shortcut ?? ""
                            color: Theme.faint
                            font { family: Theme.mono; pointSize: Theme.pt(10.5) }
                        }
                    }
                    MouseArea {
                        id: area
                        anchors.fill: parent
                        hoverEnabled: true
                        enabled: modelData.enabled !== false
                        onClicked: { menu.close(); modelData.run() }
                    }
                }
            }
        }
    }
}
