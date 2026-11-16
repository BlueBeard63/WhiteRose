// Settings › Activities (Figma "Settings - Activities"): turning
// activities on, and the activities themselves: each with its wallpaper,
// name (click to rename in place) and windows; delete one (its windows
// move to the current activity; the first, the default, stays) or add
// one. The compositor keeps each activity's windows (activities.rs);
// SUPER+SHIFT+A switches.
import QtQuick
import qs
import qs.settings
import "Activities.js" as A

Column {
    id: page
    spacing: 16

    // (As plain JS: a setting's nested lists come as Qt lists.)
    readonly property var list: JSON.parse(JSON.stringify(Daemon.setting("activities.list", [{ id: "default", name: "Main" }]) ?? []))
    readonly property bool on: Daemon.setting("activities.enabled", false)
    // Window counts from the compositor, by id.
    readonly property var counts: {
        const m = {};
        for (const a of Compositor.activities?.activities ?? []) m[a.id] = a.windows;
        return m;
    }

    PageHeader { title: "Activities"; subtitle: "Separate sets of workspaces, dock pins, wallpaper and recent files." }

    Section { text: "Activities" }
    Card {
        SettingRow {
            divider: false
            title: "Activities"
            subtitle: "SUPER+SHIFT+A switches between them; the bar shows the current one"
            Toggle { checked: page.on; onToggled: v => Daemon.set("activities.enabled", v) }
        }
    }

    Section { text: "Your activities"; visible: page.on }
    Card {
        visible: page.on
        Repeater {
            model: page.list
            delegate: Item {
                id: row
                required property var modelData
                required property int index
                readonly property bool here: modelData.id === Activities.current
                property bool renaming: false
                width: parent?.width ?? 0
                height: 56
                Rectangle { visible: row.index > 0; width: parent.width; height: 1; color: Theme.surface2 }

                Rectangle {
                    id: thumb
                    x: 14
                    anchors.verticalCenter: parent.verticalCenter
                    width: 48; height: 28; radius: 5
                    clip: true
                    color: Theme.surface2
                    WallpaperView {
                        anchors.fill: parent
                        wallpaper: Activities.settingFor(row.modelData.id, "wallpaper.current", "builtin:rose")
                        sourceSize: Qt.size(96, 56)
                    }
                }
                Column {
                    x: thumb.x + thumb.width + 14
                    anchors.verticalCenter: parent.verticalCenter
                    width: trash.x - x - 14
                    spacing: 2
                    Item {
                        width: parent.width
                        height: nameText.implicitHeight
                        Text {
                            id: nameText
                            visible: !row.renaming
                            text: row.modelData.name
                            elide: Text.ElideRight
                            width: Math.min(implicitWidth, parent.width)
                            color: Theme.text
                            font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.Medium }
                            Accessible.role: Accessible.Button
                            Accessible.name: "Rename " + row.modelData.name
                            Accessible.onPressAction: row.startRename()
                            MouseArea { anchors.fill: parent; cursorShape: Qt.IBeamCursor; onClicked: row.startRename() }
                        }
                        TextInput {
                            id: nameInput
                            visible: row.renaming
                            width: parent.width
                            color: Theme.text
                            selectionColor: Theme.rose
                            selectedTextColor: Theme.bg
                            maximumLength: 40
                            font: nameText.font
                            Accessible.role: Accessible.EditableText
                            Accessible.name: "Name"
                            onAccepted: row.finishRename(true)
                            onActiveFocusChanged: if (!activeFocus && row.renaming) row.finishRename(true)
                            Keys.onEscapePressed: row.finishRename(false)
                        }
                    }
                    Text {
                        text: A.subtitle(page.counts[row.modelData.id], row.here)
                        color: Theme.muted
                        font { family: Theme.font; pointSize: Theme.pt(11.5) }
                    }
                }
                function startRename() {
                    nameInput.text = row.modelData.name;
                    row.renaming = true;
                    nameInput.forceActiveFocus();
                    nameInput.selectAll();
                }
                function finishRename(keep) {
                    row.renaming = false;
                    if (!keep) return;
                    const next = A.rename(page.list, row.modelData.id, nameInput.text);
                    if (next) Daemon.set("activities.list", next);
                }
                Icon {
                    id: trash
                    // (The default activity stays: it holds the 0.3 settings.)
                    visible: row.modelData.id !== "default"
                    x: parent.width - 16 - width
                    anchors.verticalCenter: parent.verticalCenter
                    name: "trash"; size: 14
                    color: trashMouse.containsMouse ? Theme.rose : Theme.faint
                    Accessible.role: Accessible.Button
                    Accessible.name: "Delete " + row.modelData.name
                    Accessible.onPressAction: Daemon.set("activities.list", A.remove(page.list, row.modelData.id))
                    MouseArea {
                        id: trashMouse
                        anchors.fill: parent
                        anchors.margins: -8
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: Daemon.set("activities.list", A.remove(page.list, row.modelData.id))
                    }
                }
            }
        }
        Item {
            width: parent?.width ?? 0
            height: 46
            Rectangle { width: parent.width; height: 1; color: Theme.surface2 }
            Row {
                x: 14
                anchors.verticalCenter: parent.verticalCenter
                spacing: 8
                Icon { anchors.verticalCenter: parent.verticalCenter; name: "plus"; size: 13; color: Theme.rose }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: "Add activity"
                    color: Theme.rose
                    font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.Medium }
                }
            }
            Accessible.role: Accessible.Button
            Accessible.name: "Add activity"
            Accessible.onPressAction: Daemon.set("activities.list", A.add(page.list, Date.now()))
            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: Daemon.set("activities.list", A.add(page.list, Date.now()))
            }
        }
    }
    Text {
        visible: page.on
        width: parent.width
        wrapMode: Text.Wrap
        text: "Click a name to rename it. An activity's wallpaper is the one set in Appearance while it's the current one. Deleting one moves its windows to the current activity."
        color: Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(11.5) }
    }
}
