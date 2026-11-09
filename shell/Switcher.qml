// ALT+TAB (Figma: Application Switcher) and SUPER+ALT+TAB (Figma: Window
// Switcher). The compositor runs the selection and keys; this only draws it.
import QtQuick
import Quickshell
import Quickshell.Wayland

PanelWindow {
    id: root
    readonly property var sw: Compositor.switcher
    readonly property bool windowsMode: sw?.mode === "windows"
    // Stays mapped briefly while closing so the island can retract. A plain
    // timer, not the animation's `running`: tying visibility to the animation
    // (whose target depends on this window's contents) made a binding loop.
    visible: sw !== null || retracting.running
    Timer { id: retracting; interval: 200 }

    anchors { top: true; left: true; right: true }
    exclusionMode: ExclusionMode.Normal
    implicitHeight: shape.height + 4
    color: "transparent"
    WlrLayershell.namespace: "whiterose-switcher"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
    mask: Region { item: shape }

    // Keep the last state while retracting, so content doesn't vanish first.
    property var shown: null
    onSwChanged: if (sw) shown = sw; else retracting.restart()

    function entryFor(appId) { return appId ? DesktopEntries.heuristicLookup(appId) : null }
    function nameFor(appId) { return entryFor(appId)?.name ?? appId ?? "" }

    Item {
        id: slider
        anchors.horizontalCenter: parent.horizontalCenter
        width: shape.width
        height: shape.height
        // Slide with a transform, not `y`: the window's size follows this item's
        // height, so animating its position through layout made a binding loop.
        transform: Translate {
            y: root.sw ? 0 : -slider.height
            Behavior on y { NumberAnimation { duration: 180; easing.type: Easing.OutCubic } }
        }

        PanelShape {
            id: shape
            bodyWidth: content.width + 48
            bodyHeight: content.height + 38
            earWidth: 22
            earHeight: 26
            bottomLeftRadius: 34
            bottomRightRadius: 34
        }

        Column {
            id: content
            x: shape.bodyX + 24
            y: 16
            spacing: 14

            // ---- SUPER+ALT+TAB: windows ---------------------------------
            Row {
                id: cards
                visible: root.windowsMode
                spacing: 14
                // As many cards as fit; scrolled so the selection stays visible.
                readonly property int fit: Math.max(1, Math.floor((root.width - 160) / (206 + 14)))
                readonly property int count: root.shown?.items.length ?? 0
                readonly property int sel: root.shown?.selected ?? 0
                readonly property int first: Math.max(0, Math.min(sel - Math.floor(fit / 2), count - fit))
                Repeater {
                    model: root.windowsMode && root.shown ? root.shown.items.slice(cards.first, cards.first + cards.fit) : []
                    delegate: Rectangle {
                        id: card
                        required property var modelData
                        required property int index
                        readonly property int windowId: modelData.windows[0]
                        readonly property var win: Compositor.windowById(windowId)
                        readonly property bool selected: cards.first + index === cards.sel
                        width: 206
                        height: cardCol.implicitHeight + 16
                        radius: 18
                        color: selected ? Theme.surface1 : "transparent"
                        border { width: selected ? 1.5 : 0; color: Theme.pearl }
                        Column {
                            id: cardCol
                            x: 8; y: 8
                            width: parent.width - 16
                            spacing: 10
                            WindowPreview {
                                anchors.horizontalCenter: parent.horizontalCenter
                                toplevel: Compositor.toplevelFor(card.windowId)
                                maxWidth: 190
                                maxHeight: 118
                                showTitle: false
                                highlighted: false
                                onActivated: Compositor.focusWindow(card.windowId)
                            }
                            Row {
                                width: parent.width
                                spacing: 7
                                Image {
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: 14; height: 14
                                    source: Quickshell.iconPath(root.entryFor(card.win?.app_id)?.icon ?? "", true)
                                    sourceSize: Qt.size(28, 28)
                                }
                                Text {
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: parent.width - 14 - 7 - wsChip.width - 7
                                    text: card.win?.title || root.nameFor(card.win?.app_id)
                                    elide: Text.ElideRight
                                    color: card.selected ? Theme.text : Theme.muted
                                    font { family: Theme.font; pointSize: Theme.pt(12.5); weight: Font.Medium }
                                }
                                Rectangle {
                                    id: wsChip
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: wsText.implicitWidth + 12; height: 18; radius: 5
                                    color: Theme.surface2
                                    Text {
                                        id: wsText
                                        anchors.centerIn: parent
                                        text: card.win?.workspace ?? ""
                                        color: Theme.muted
                                        font { family: Theme.mono; pointSize: Theme.pt(10.5); weight: Font.Medium }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ---- ALT+TAB: applications ----------------------------------
            Row {
                id: apps
                visible: !root.windowsMode
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 10
                Repeater {
                    model: !root.windowsMode && root.shown ? root.shown.items : []
                    delegate: Item {
                        id: app
                        required property var modelData
                        required property int index
                        readonly property bool selected: index === (root.shown?.selected ?? -1)
                        width: 66; height: 66
                        Rectangle {
                            anchors.fill: parent
                            radius: 18
                            color: app.selected ? Theme.surface2 : Theme.surface1
                            border { width: app.selected ? 1.5 : 0; color: Theme.pearl }
                            Image {
                                id: appIcon
                                anchors.centerIn: parent
                                width: 34; height: 34
                                source: Quickshell.iconPath(root.entryFor(app.modelData.app_id)?.icon ?? "", true)
                                sourceSize: Qt.size(68, 68)
                                visible: status === Image.Ready
                            }
                            Text {
                                anchors.centerIn: parent
                                visible: appIcon.status !== Image.Ready
                                text: root.nameFor(app.modelData.app_id).charAt(0).toUpperCase()
                                color: Theme.text
                                font { family: Theme.font; pointSize: Theme.pt(20); weight: Font.DemiBold }
                            }
                            MouseArea {
                                anchors.fill: parent
                                cursorShape: Qt.PointingHandCursor
                                onClicked: Compositor.focusWindow(app.modelData.windows[0])
                            }
                        }
                        // Window count, when there's more than one.
                        Rectangle {
                            visible: app.modelData.windows.length > 1
                            x: parent.width - 14; y: -4
                            width: 18; height: 18; radius: 9
                            color: Theme.rose
                            border { width: 2; color: Theme.bg }
                            Text {
                                anchors.centerIn: parent
                                text: app.modelData.windows.length
                                color: Theme.bg
                                font { family: Theme.font; pointSize: Theme.pt(10.5); weight: Font.DemiBold }
                            }
                        }
                    }
                }
            }
            Column {
                visible: !root.windowsMode
                anchors.horizontalCenter: parent.horizontalCenter
                spacing: 2
                readonly property var item: root.shown?.items[root.shown.selected] ?? null
                readonly property var target: item ? Compositor.windowById(item.windows[root.shown.sub] ?? item.windows[0]) : null
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    text: root.nameFor(parent.item?.app_id)
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
                }
                Text {
                    anchors.horizontalCenter: parent.horizontalCenter
                    readonly property int n: parent.item?.windows.length ?? 0
                    text: `${n} window${n === 1 ? "" : "s"}` + (parent.target?.title ? ` · ${parent.target.title}` : "")
                    color: Theme.muted
                    elide: Text.ElideRight
                    width: Math.min(implicitWidth, 420)
                    font { family: Theme.font; pointSize: Theme.pt(11.5) }
                }
            }
        }
    }
}
