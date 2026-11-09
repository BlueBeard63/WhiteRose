// A dropdown menu in the desktop's style: `items` is [{ label, shortcut?,
// danger?, enabled?, checked? (a tick: on/off items), separator?, run }]. Open it with openAt(x, y) (in its
// parent's coordinates) or open().
import QtQuick
import QtQuick.Controls

Popup {
    id: menu
    property var items: []
    function openAt(px, py) { menu.x = px; menu.y = py; menu.open() }
    padding: 6
    // As wide as the widest label plus its shortcut.
    width: {
        let w = 200;
        for (const c of list.children) if (c.need) w = Math.max(w, c.need + 12);
        return w;
    }
    background: Rectangle { color: Theme.surface1; radius: 12; border { width: 1; color: Theme.surface2 } }
    contentItem: Column {
        id: list
        spacing: 2
        Repeater {
            model: menu.items
            delegate: Item {
                required property var modelData
                readonly property real need: labelText.implicitWidth + keys.implicitWidth + (keys.text ? 56 : 24) + (tick.visible ? 20 : 0)
                width: menu.width - 12
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
                        Item {
                            id: tick
                            visible: modelData.checked !== undefined
                            width: 20; height: 14
                            anchors.verticalCenter: parent.verticalCenter
                            Icon { visible: modelData.checked === true; name: "check"; size: 13; color: Theme.rose; anchors.verticalCenter: parent.verticalCenter }
                        }
                        Text {
                            id: labelText
                            width: parent.width - keys.width - (tick.visible ? tick.width : 0)
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
