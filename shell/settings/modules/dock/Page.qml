// Dock: which apps are pinned, in what order. Add from the list of
// installed apps; changes show in the dock at once.
import QtQuick
import Quickshell
import qs
import qs.settings

Column {
    id: page
    spacing: 16

    readonly property var pinned: Daemon.setting("dock.pinned", [])
    function save(list) { Daemon.set("dock.pinned", list) }
    function move(i, d) {
        const l = page.pinned.slice();
        const j = i + d;
        if (j < 0 || j >= l.length) return;
        [l[i], l[j]] = [l[j], l[i]];
        save(l);
    }
    readonly property var apps: DesktopEntries.applications.values.filter(e => !e.noDisplay)
        .sort((a, b) => a.name.localeCompare(b.name))
    property string filter: ""
    readonly property var addable: apps.filter(e => !page.pinned.includes(e.id) && (filter === "" || e.name.toLowerCase().includes(filter.toLowerCase())))

    PageHeader { title: "Dock"; subtitle: "Pinned apps, in order. Running apps that aren't pinned still show while they're open." }

    Section { text: "Pinned" }
    Card {
        SettingRow {
            visible: page.pinned.length === 0
            divider: false
            title: "Nothing pinned"
        }
        Repeater {
            model: page.pinned
            delegate: Item {
                id: pin
                required property string modelData
                required property int index
                readonly property var entry: DesktopEntries.byId(modelData)
                width: parent.width; height: 52
                Rectangle { visible: pin.index > 0; width: parent.width; height: 1; color: Theme.surface2 }
                AppIcon { x: 14; anchors.verticalCenter: parent.verticalCenter; size: 26; entry: pin.entry; name: pin.entry?.name ?? pin.modelData }
                Column {
                    x: 52; anchors.verticalCenter: parent.verticalCenter
                    Text { text: pin.entry?.name ?? pin.modelData; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.Medium } }
                    Text { visible: pin.entry === null; text: "Not installed"; color: Theme.rose; font { family: Theme.font; pointSize: Theme.pt(11) } }
                }
                Row {
                    anchors { right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter }
                    spacing: 4
                    IconButton { width: 28; height: 28; icon: "chevUp"; iconSize: 13; fill: "transparent"; visible: pin.index > 0; onClicked: page.move(pin.index, -1) }
                    IconButton { width: 28; height: 28; icon: "chevDown"; iconSize: 13; fill: "transparent"; visible: pin.index < page.pinned.length - 1; onClicked: page.move(pin.index, 1) }
                    IconButton {
                        width: 28; height: 28; icon: "x"; iconSize: 12; fill: "transparent"; hoverFill: Theme.rose
                        iconColor: hovered ? Theme.bg : Theme.muted
                        onClicked: page.save(page.pinned.filter(p => p !== pin.modelData))
                    }
                }
            }
        }
    }

    Item {
        width: parent.width; height: 36
        Section { anchors.verticalCenter: parent.verticalCenter; text: "Add an app" }
        Field { anchors.right: parent.right; implicitWidth: 220; placeholder: "Filter"; onTextChanged: page.filter = text }
    }
    Card {
        Flickable {
            width: parent.width
            height: Math.min(addCol.height, 320)
            contentHeight: addCol.height
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            Column {
                id: addCol
                width: parent.width
                Repeater {
                    model: page.addable
                    delegate: Rectangle {
                        required property var modelData
                        width: parent.width; height: 44
                        color: addMouse.containsMouse ? Theme.surface2 : "transparent"
                        AppIcon { x: 14; anchors.verticalCenter: parent.verticalCenter; size: 22; entry: modelData; name: modelData.name }
                        Text { x: 48; anchors.verticalCenter: parent.verticalCenter; text: modelData.name; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(12.5) } }
                        Icon { anchors { right: parent.right; rightMargin: 16; verticalCenter: parent.verticalCenter } name: "plus"; size: 14; color: Theme.muted }
                        MouseArea { id: addMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: page.save([...page.pinned, modelData.id]) }
                    }
                }
            }
        }
    }
}
