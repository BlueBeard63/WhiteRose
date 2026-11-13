// Files: the file manager's settings, in six parts (the sub-pages under
// Files in the sidebar). This page points to each.
import QtQuick
import qs
import qs.settings

Column {
    id: page
    spacing: 16
    readonly property var parts: [
        { id: "general", icon: "gear", name: "General", text: "Where Files starts, single or double click, the trash, hidden files" },
        { id: "views", icon: "grid", name: "Views & lists", text: "How folders look until you choose: view, sort, groups, sizes, columns" },
        { id: "search", icon: "search", name: "Search", text: "What the search index covers, and making it again" },
        { id: "apps", icon: "terminal", name: "Open with", text: "The terminal and code editor for “Open in…”" },
        { id: "menus", icon: "menu", name: "Toolbar & menus", text: "Extra toolbar buttons, what the right-click menu shows, and your own actions" },
        { id: "shortcuts", icon: "keyboard", name: "Shortcuts", text: "Every Files shortcut, and changing them" },
    ]

    PageHeader { title: "Files"; subtitle: "The file manager: how it starts, shows folders, searches and opens things." }

    Card {
        Repeater {
            model: page.parts
            delegate: Item {
                id: part
                required property var modelData
                required property int index
                width: parent.width; height: 60
                Rectangle { visible: part.index > 0; width: parent.width; height: 1; color: Theme.surface2 }
                Rectangle {
                    anchors { fill: parent; margins: 4 }
                    radius: 10
                    color: partArea.containsMouse ? Theme.surface2 : "transparent"
                }
                Rectangle {
                    id: badge
                    x: 14; anchors.verticalCenter: parent.verticalCenter
                    width: 34; height: 34; radius: 10
                    color: Theme.surface2
                    Icon { anchors.centerIn: parent; name: part.modelData.icon; size: 16; color: Theme.rose }
                }
                Column {
                    anchors { left: badge.right; leftMargin: 12; right: chev.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
                    spacing: 2
                    Text { text: part.modelData.name; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.Medium } }
                    Text { width: parent.width; text: part.modelData.text; elide: Text.ElideRight; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
                }
                Icon { id: chev; anchors { right: parent.right; rightMargin: 16; verticalCenter: parent.verticalCenter } name: "chevR"; size: 14; color: Theme.faint }
                MouseArea { id: partArea; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: SettingsState.open("files/" + part.modelData.id) }
            }
        }
    }
}
