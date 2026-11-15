// A dropdown menu in the desktop's style: `items` is [{ label, shortcut?,
// danger?, enabled?, checked? (a tick: on/off items), colour? (a dot
// before the label), separator?, run }].
// An item with `items` of its own (and no run) is a submenu: › on it, and
// it opens beside it on hover or click (to the left if there's no room on
// the right). Open with openAt(x, y) (in its parent's coordinates) or open().
// An item with `stepper` ({ value, minus, plus }) shows − value + buttons
// that act without closing the menu.
import QtQuick
import QtQuick.Controls
import QtQuick.Window

Popup {
    id: menu
    property var items: []
    function openAt(px, py) { menu.x = px; menu.y = py; menu.open() }
    // Never off the window: up (and left) as far as it takes to fit.
    margins: 6
    // Something in it (or a submenu of it) was chosen.
    signal chosen()
    // The submenu open (its index; -1 none).
    property int openIndex: -1
    onClosed: menu.openIndex = -1
    Timer { id: subTimer; interval: 160; property int want: -1; onTriggered: menu.openIndex = want }
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
                id: entry
                required property var modelData
                required property int index
                readonly property bool sub: !!modelData.items
                // (For screen readers and the UI tests.)
                Accessible.role: modelData.separator ? Accessible.Separator : Accessible.MenuItem
                Accessible.name: modelData.label ?? ""
                readonly property real need: (modelData.stepper ? 110 : 0) + labelText.implicitWidth + keys.implicitWidth + (keys.text ? 56 : 24) + (tick.visible ? 20 : 0) + (sub ? 20 : 0) + (dot.visible ? 18 : 0)
                width: menu.width - 12
                height: modelData.separator ? 9 : 30
                Rectangle { visible: !!modelData.separator; anchors.centerIn: parent; width: parent.width - 8; height: 1; color: Theme.surface2 }
                Rectangle {
                    visible: !modelData.separator
                    anchors.fill: parent
                    radius: 8
                    color: (area.containsMouse || menu.openIndex === entry.index && entry.sub) && modelData.enabled !== false ? Theme.surface2 : "transparent"
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
                        Item {
                            id: dot
                            visible: !!modelData.colour
                            width: 18; height: 14
                            anchors.verticalCenter: parent.verticalCenter
                            Rectangle { width: 9; height: 9; radius: 4.5; anchors.verticalCenter: parent.verticalCenter; color: modelData.colour ?? "transparent" }
                        }
                        Text {
                            id: labelText
                            width: parent.width - keys.width - (tick.visible ? tick.width : 0) - (entry.sub ? 14 : 0) - (dot.visible ? dot.width : 0)
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
                        Icon { visible: entry.sub; name: "chevR"; size: 12; color: Theme.muted; anchors.verticalCenter: parent.verticalCenter }
                    }
                    // − value +
                    Row {
                        visible: !!modelData.stepper
                        anchors { right: parent.right; rightMargin: 6; verticalCenter: parent.verticalCenter }
                        spacing: 2
                        z: 2
                        IconButton { width: 24; height: 24; icon: "minus"; iconSize: 11; onClicked: modelData.stepper.minus() }
                        Text {
                            width: 44
                            anchors.verticalCenter: parent.verticalCenter
                            horizontalAlignment: Text.AlignHCenter
                            text: modelData.stepper?.value ?? ""
                            color: Theme.text
                            font { family: Theme.mono; pointSize: Theme.pt(10.5) }
                        }
                        IconButton { width: 24; height: 24; icon: "plus"; iconSize: 11; onClicked: modelData.stepper.plus() }
                    }
                    MouseArea {
                        id: area
                        anchors.fill: parent
                        visible: !modelData.stepper
                        hoverEnabled: true
                        enabled: modelData.enabled !== false
                        // Hovering: its submenu opens (another's closes) after a moment.
                        onContainsMouseChanged: if (containsMouse) { subTimer.want = entry.sub ? entry.index : -1; subTimer.restart() }
                        onClicked: {
                            if (entry.sub) { subTimer.stop(); menu.openIndex = entry.index; return }
                            // The action runs once every menu has closed (it may
                            // change what the menus hold, and they go with it). It's
                            // queued first: closing a parent menu destroys its
                            // submenus, this one included, mid-click.
                            const run = modelData.run;
                            Qt.callLater(run);
                            menu.close();
                            menu.chosen();
                        }
                    }
                    // The submenu (made when first opened; Menu.qml can't hold itself directly).
                    Loader {
                        id: subLoader
                        active: entry.sub && menu.openIndex === entry.index
                        source: active ? Qt.resolvedUrl("Menu.qml") : ""
                        onLoaded: {
                            const m = subLoader.item;
                            m.items = entry.modelData.items;
                            // Right of the menu, or left if it would run off the window.
                            const right = entry.mapToItem(null, entry.width, 0).x + 10;
                            const room = (entry.Window.window?.width ?? 10000) - right;
                            m.x = room >= m.width ? entry.width + 10 : -m.width - 10;
                            m.y = -6;
                            // (Held here: by the time these run, this delegate may be gone.)
                            const owner = menu, at = entry.index;
                            m.chosen.connect(() => { owner.close(); owner.chosen() });
                            m.closed.connect(() => { if (owner.openIndex === at) owner.openIndex = -1 });
                            m.open();
                        }
                    }
                }
            }
        }
    }
}
