// Island view: the activity picker (Figma "Activities - SUPER + SHIFT +
// A"). The compositor runs the choice and the keys (activities.rs); this
// draws each activity as a card: its wallpaper with its shown
// workspace's windows sketched on it, its name and how many windows it
// has, the current one marked; then a card for a new activity. A click
// on a card goes there.
import QtQuick
import QtQuick.Shapes
import Quickshell
import Quickshell.Widgets

Item {
    id: root
    readonly property var state: Compositor.activities
    // Keep the last state while the island morphs back.
    property var shown: state
    onStateChanged: if (state && state.picker !== null && state.picker !== undefined) shown = state
    readonly property var cards: root.shown?.activities ?? []
    readonly property int selected: root.shown?.picker ?? -1

    implicitWidth: content.width + 48
    implicitHeight: content.height + 38

    Column {
        id: content
        x: 24
        y: 16
        spacing: 14

        Row {
            spacing: 8
            Text {
                text: "Activities"
                color: Theme.text
                font { family: Theme.font; pointSize: Theme.pt(13.5); weight: Font.DemiBold }
            }
            Text {
                anchors.baseline: parent.children[0].baseline
                text: `${root.cards.length} ${root.cards.length === 1 ? "activity" : "activities"} · SUPER+SHIFT+A again for the next`
                color: Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(11) }
            }
        }

        Row {
            spacing: 14
            Repeater {
                model: root.cards
                delegate: Rectangle {
                    id: card
                    required property var modelData
                    required property int index
                    readonly property bool chosen: index === root.selected
                    readonly property bool here: modelData.id === (root.shown?.current ?? "")
                    readonly property string name: Activities.nameOf(modelData.id)
                    Accessible.role: Accessible.Button
                    Accessible.name: "Activity " + name
                    Accessible.selected: chosen
                    Accessible.onPressAction: Compositor.goToActivity(modelData.id)
                    width: 206
                    height: cardCol.implicitHeight + 16
                    radius: 18
                    color: chosen ? Theme.surface1 : "transparent"
                    border { width: chosen ? 1.5 : 0; color: Theme.pearl }

                    Column {
                        id: cardCol
                        x: 8; y: 8
                        width: parent.width - 16
                        spacing: 10
                        ClippingRectangle {
                            width: 190; height: 118
                            radius: 10
                            color: Theme.surface1
                            WallpaperView {
                                anchors.fill: parent
                                wallpaper: Activities.settingFor(card.modelData.id, "wallpaper.current", "builtin:rose")
                                sourceSize: Qt.size(380, 236)
                            }
                            // Its windows, sketched where they are.
                            Repeater {
                                model: card.modelData.places
                                delegate: Rectangle {
                                    required property var modelData
                                    x: modelData[0] * 190; y: modelData[1] * 118
                                    width: Math.max(4, modelData[2] * 190); height: Math.max(4, modelData[3] * 118)
                                    radius: 4
                                    color: Theme.surface1
                                    border { width: 1; color: Qt.rgba(1, 1, 1, 0.06) }
                                    Rectangle {
                                        width: parent.width; height: Math.min(8, parent.height)
                                        radius: 4
                                        color: Theme.surface2
                                    }
                                }
                            }
                            Rectangle {
                                visible: card.here
                                anchors { top: parent.top; right: parent.right; margins: 6 }
                                width: currentText.implicitWidth + 12; height: 18; radius: 5
                                color: Qt.rgba(0.05, 0.05, 0.06, 0.85)
                                Text {
                                    id: currentText
                                    anchors.centerIn: parent
                                    text: "Current"
                                    color: Theme.text
                                    font { family: Theme.font; pointSize: Theme.pt(10); weight: Font.Medium }
                                }
                            }
                        }
                        Row {
                            width: parent.width
                            spacing: 7
                            Text {
                                anchors.verticalCenter: parent.verticalCenter
                                width: parent.width - countChip.width - 7
                                leftPadding: 2
                                text: card.name
                                elide: Text.ElideRight
                                color: card.chosen ? Theme.text : Theme.muted
                                font { family: Theme.font; pointSize: Theme.pt(12.5); weight: Font.Medium }
                            }
                            Rectangle {
                                id: countChip
                                anchors.verticalCenter: parent.verticalCenter
                                width: countText.implicitWidth + 12; height: 18; radius: 5
                                color: Theme.surface2
                                Text {
                                    id: countText
                                    anchors.centerIn: parent
                                    text: card.modelData.windows
                                    color: Theme.muted
                                    font { family: Theme.mono; pointSize: Theme.pt(10.5); weight: Font.Medium }
                                }
                            }
                        }
                    }
                    MouseArea {
                        anchors.fill: parent
                        cursorShape: Qt.PointingHandCursor
                        onClicked: Compositor.goToActivity(card.modelData.id)
                    }
                }
            }

            // A new activity.
            Rectangle {
                id: newCard
                readonly property bool chosen: root.selected === root.cards.length
                Accessible.role: Accessible.Button
                Accessible.name: "New activity"
                Accessible.selected: chosen
                Accessible.onPressAction: Compositor.newActivity()
                width: 206
                height: newCol.implicitHeight + 16
                radius: 18
                color: chosen ? Theme.surface1 : "transparent"
                border { width: chosen ? 1.5 : 0; color: Theme.pearl }
                Column {
                    id: newCol
                    x: 8; y: 8
                    width: parent.width - 16
                    spacing: 10
                    Item {
                        width: 190; height: 118
                        Shape {
                            anchors.fill: parent
                            ShapePath {
                                strokeColor: Qt.rgba(1, 1, 1, 0.14)
                                strokeWidth: 1.5
                                strokeStyle: ShapePath.DashLine
                                dashPattern: [3.5, 2.5]
                                fillColor: "transparent"
                                startX: 10; startY: 1
                                PathLine { x: 180; y: 1 }
                                PathArc { x: 189; y: 10; radiusX: 9; radiusY: 9 }
                                PathLine { x: 189; y: 108 }
                                PathArc { x: 180; y: 117; radiusX: 9; radiusY: 9 }
                                PathLine { x: 10; y: 117 }
                                PathArc { x: 1; y: 108; radiusX: 9; radiusY: 9 }
                                PathLine { x: 1; y: 10 }
                                PathArc { x: 10; y: 1; radiusX: 9; radiusY: 9 }
                            }
                        }
                        Icon { anchors.centerIn: parent; name: "plus"; size: 20; color: Theme.muted }
                    }
                    Text {
                        leftPadding: 2
                        text: "New activity"
                        color: newCard.chosen ? Theme.text : Theme.muted
                        font { family: Theme.font; pointSize: Theme.pt(12.5); weight: Font.Medium }
                    }
                }
                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: Compositor.newActivity()
                }
            }
        }
    }
}
