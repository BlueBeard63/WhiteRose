// The window menu (Figma "Window menu - right-click on a title bar"), from
// the compositor's `window_menu` event (window_menu.rs): an app's title
// bar right-clicked, or ALT+SPACE. Move the window to a workspace, put it
// on another activity or on every one (while activities are on), close
// it. Esc or a click elsewhere closes the menu; so does picking anything.
import QtQuick
import Quickshell
import Quickshell.Wayland

Scope {
    id: root
    readonly property var menu: Compositor.windowMenu
    readonly property var win: menu ? Compositor.windowById(menu.id) : null
    readonly property bool everywhere: menu !== null && (Compositor.activities?.everywhere ?? []).includes(menu.id)
    readonly property var screen: menu ? Quickshell.screens.find(s => menu.x >= s.x && menu.x < s.x + s.width
        && menu.y >= s.y && menu.y < s.y + s.height) ?? Quickshell.screens[0] : null
    // What closes a window, as Settings › Keyboard has it.
    readonly property string closeKeys: (Compositor.shortcuts.find(s => s.id === "close-window")?.keys ?? [])[0] ?? ""

    function done() { Compositor.closeWindowMenu() }

    LazyLoader {
        active: root.menu !== null && root.screen !== null
        PanelWindow {
            id: win
            screen: root.screen
            anchors { top: true; bottom: true; left: true; right: true }
            exclusionMode: ExclusionMode.Ignore
            color: "transparent"
            WlrLayershell.layer: WlrLayer.Overlay
            WlrLayershell.namespace: "whiterose-window-menu"
            LayerTitle { name: "whiterose-window-menu" }
            WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive

            // A click anywhere else closes it.
            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.LeftButton | Qt.RightButton | Qt.MiddleButton
                onPressed: root.done()
            }
            Item {
                focus: true
                Keys.onEscapePressed: root.done()
            }

            Rectangle {
                id: box
                Accessible.role: Accessible.PopupMenu
                Accessible.name: "Window menu"
                // Where it was asked for, kept on the screen.
                x: root.menu ? Math.max(8, Math.min(root.menu.x - root.screen.x, win.width - width - 8)) : 0
                y: root.menu ? Math.max(Theme.barHeight + 4, Math.min(root.menu.y - root.screen.y, win.height - height - 8)) : 0
                width: 232
                height: items.implicitHeight + 12
                radius: 12
                color: Theme.surface1
                border { width: 1; color: Qt.rgba(1, 1, 1, 0.07) }
                // Clicks inside don't reach the close-everywhere area.
                MouseArea { anchors.fill: parent; acceptedButtons: Qt.AllButtons }

                Column {
                    id: items
                    x: 6; y: 6
                    width: parent.width - 12

                    component Head: Text {
                        leftPadding: 10; topPadding: 8; bottomPadding: 4
                        color: Theme.faint
                        font { family: Theme.font; pointSize: Theme.pt(9.5); weight: Font.Medium; letterSpacing: 0.6 }
                    }
                    component Line: Item {
                        width: items.width; height: 11
                        Rectangle { anchors.verticalCenter: parent.verticalCenter; width: parent.width; height: 1; color: Qt.rgba(1, 1, 1, 0.06) }
                    }
                    component Entry: Rectangle {
                        id: entry
                        property string label
                        property string hint: ""
                        property bool checked: false
                        signal picked()
                        Accessible.role: Accessible.MenuItem
                        Accessible.name: label
                        Accessible.checked: checked
                        Accessible.onPressAction: entry.picked()
                        width: items.width; height: 30; radius: 7
                        color: mouse.containsMouse ? Qt.rgba(1, 1, 1, 0.07) : "transparent"
                        Text {
                            x: 10; width: 14
                            anchors.verticalCenter: parent.verticalCenter
                            text: entry.checked ? "✓" : ""
                            color: Theme.rose
                            font { family: Theme.font; pointSize: Theme.pt(11); weight: Font.Medium }
                        }
                        Text {
                            x: 30
                            anchors.verticalCenter: parent.verticalCenter
                            text: entry.label
                            color: Theme.text
                            font { family: Theme.font; pointSize: Theme.pt(12) }
                        }
                        Text {
                            anchors { right: parent.right; rightMargin: 10; verticalCenter: parent.verticalCenter }
                            text: entry.hint
                            color: Theme.faint
                            font { family: Theme.font; pointSize: Theme.pt(10.5) }
                        }
                        MouseArea {
                            id: mouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: entry.picked()
                        }
                    }

                    Head { text: "WORKSPACE" }
                    Row {
                        leftPadding: 6; bottomPadding: 4
                        spacing: 2
                        Repeater {
                            model: Compositor.workspaceCount
                            delegate: Rectangle {
                                id: chip
                                required property int index
                                readonly property int n: index + 1
                                readonly property bool here: root.win?.workspace === n
                                Accessible.role: Accessible.Button
                                Accessible.name: "Workspace " + n
                                Accessible.onPressAction: chip.pick()
                                function pick() {
                                    if (!chip.here) Compositor.moveWindow(root.menu.id, chip.n);
                                    root.done();
                                }
                                width: 22; height: 22; radius: 6
                                color: here ? Theme.pearl : chipMouse.containsMouse ? Qt.rgba(1, 1, 1, 0.07) : "transparent"
                                Text {
                                    anchors.centerIn: parent
                                    text: chip.n
                                    color: chip.here ? Theme.bg : Theme.muted
                                    font { family: Theme.font; pointSize: Theme.pt(10.5); weight: Font.Medium }
                                }
                                MouseArea {
                                    id: chipMouse
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: Qt.PointingHandCursor
                                    onClicked: chip.pick()
                                }
                            }
                        }
                    }

                    // Activities, while they're on.
                    Line { visible: Activities.enabled }
                    Head { visible: Activities.enabled; text: "ACTIVITY" }
                    Repeater {
                        model: Activities.enabled ? Activities.list : []
                        delegate: Entry {
                            required property var modelData
                            label: modelData.name
                            checked: !root.everywhere && modelData.id === Activities.current
                            onPicked: {
                                if (!checked) Compositor.windowActivity(root.menu.id, modelData.id);
                                root.done();
                            }
                        }
                    }
                    Entry {
                        visible: Activities.enabled
                        label: "Every activity"
                        checked: root.everywhere
                        onPicked: {
                            Compositor.windowActivity(root.menu.id, root.everywhere ? Activities.current : null);
                            root.done();
                        }
                    }

                    Line {}
                    Entry {
                        label: "Close"
                        hint: root.closeKeys
                        onPicked: Compositor.closeWindowById(root.menu.id)
                    }
                }
            }
        }
    }
}
