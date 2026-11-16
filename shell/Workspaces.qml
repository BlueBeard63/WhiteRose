// The bar's workspaces, in one of three styles ("bar.workspaceStyle"):
// dots (Figma: Workspaces; active = wide pearl pill, occupied = muted dot,
// empty = faint dot), numbers, or the strip (Figma: "Top bar - workspace
// strip"): one cell per workspace with the icons of the apps open on it,
// the current one pearl. In the strip an app's icon dragged onto another
// cell moves that window there (rose outline on the cell it's over), a
// click on an icon goes to that window, and with "bar.workspacePreviews"
// on the pointer resting on a cell shows a live preview of that
// workspace under it. Driven by compositor IPC events.
import QtQuick
import Quickshell
import Quickshell.Wayland
import Quickshell.Widgets
import "Strip.js" as Strip

Item {
    id: root
    // The bar's screen: the preview shows that part of the workspace.
    property var screen: null

    readonly property string style: Daemon.setting("bar.workspaceStyle", "dots")
    readonly property bool strip: style === "strip"
    readonly property bool numbers: style === "numbers"
    readonly property bool previews: strip && Daemon.setting("bar.workspacePreviews", false)
    readonly property int shown: Strip.shown(Compositor.activeWorkspace, Compositor.occupiedWorkspaces)

    // An icon being dragged (its window), where (in this item), and the
    // workspace it's over (0: none).
    property var dragging: null
    property point dragAt: Qt.point(0, 0)
    property int dropOn: 0

    // The workspace the pointer is on, and the one previewed (after a
    // moment there; moving to another cell moves it straight away).
    property int hoverOn: 0
    property int previewing: 0
    // Room the preview needs under the bar (Bar.qml grows by it).
    readonly property real reserve: preview.visible ? preview.height + 12 : 0

    implicitWidth: row.implicitWidth
    implicitHeight: row.implicitHeight

    function hover(n, on) {
        if (on) {
            hoverOn = n;
            if (previewing) previewing = n;
            else previewTimer.restart();
        } else if (hoverOn === n) {
            hoverOn = 0;
            previewing = 0;
            previewTimer.stop();
        }
    }
    Timer { id: previewTimer; interval: 350; onTriggered: root.previewing = root.hoverOn }

    // The strip's cell under `x` (in this item): its workspace, or 0.
    function cellAt(x) {
        const rects = [];
        for (let i = 0; i < cells.count; i++) {
            const c = cells.itemAt(i);
            if (c) rects.push({ x: c.mapToItem(root, 0, 0).x, width: c.width });
        }
        return Strip.cellAt(rects, x) + 1;
    }

    Row {
        id: row
        anchors.verticalCenter: parent.verticalCenter
        spacing: root.strip ? 3 : 5
        leftPadding: root.strip ? 0 : 6
        rightPadding: root.strip ? 0 : 6

        // ---- dots and numbers -------------------------------------------------------
        Repeater {
            model: root.strip ? 0 : root.shown
            delegate: Rectangle {
                required property int index
                readonly property int number: index + 1
                readonly property bool isActive: Compositor.activeWorkspace === number
                readonly property bool occupied: Compositor.occupiedWorkspaces.includes(number)

                anchors.verticalCenter: parent.verticalCenter
                width: root.numbers ? Math.max(16, label.implicitWidth + 8) : isActive ? 18 : 6
                height: root.numbers ? 16 : 6
                radius: root.numbers ? 5 : 3
                color: root.numbers ? (isActive ? Theme.pearl : "transparent") : isActive ? Theme.pearl : occupied ? Theme.muted : Theme.faint
                // Numbers style: the workspace number, bold where something's open.
                Text {
                    id: label
                    visible: root.numbers
                    anchors.centerIn: parent
                    text: parent.number
                    color: parent.isActive ? Theme.bg : parent.occupied ? Theme.text : Theme.faint
                    font { family: Theme.mono; pointSize: Theme.pt(10.5); weight: parent.occupied || parent.isActive ? Font.DemiBold : Font.Normal }
                }
                Behavior on width { NumberAnimation { duration: Theme.fastAnim; easing.type: Easing.OutCubic } }
                Behavior on color { ColorAnimation { duration: Theme.fastAnim } }

                MouseArea {
                    anchors.fill: parent
                    anchors.margins: -4   // easier to hit than 6px
                    cursorShape: Qt.PointingHandCursor
                    onClicked: Compositor.workspace(parent.number)
                }
            }
        }

        // ---- the strip ------------------------------------------------------------------
        Repeater {
            id: cells
            model: root.strip ? root.shown : 0
            delegate: Rectangle {
                id: cell
                required property int index
                readonly property int number: index + 1
                readonly property bool isActive: Compositor.activeWorkspace === number
                readonly property var info: Strip.cell(Compositor.windows, number)
                readonly property bool target: root.dragging !== null && root.dropOn === number && root.dragging.workspace !== number

                Accessible.role: Accessible.Button
                Accessible.name: "Workspace " + number
                Accessible.onPressAction: Compositor.workspace(number)

                anchors.verticalCenter: parent.verticalCenter
                height: 18
                radius: 6
                width: content.implicitWidth + (info.windows.length ? 10 : 12)
                color: target ? Qt.rgba(Theme.rose.r, Theme.rose.g, Theme.rose.b, 0.12)
                    : isActive ? Theme.pearl
                    : hoverHandler.hovered ? Qt.rgba(1, 1, 1, 0.08) : "transparent"
                border { width: target ? 1.5 : 0; color: Theme.rose }
                Behavior on color { ColorAnimation { duration: Theme.fastAnim } }

                HoverHandler {
                    id: hoverHandler
                    onHoveredChanged: root.hover(cell.number, hovered)
                }
                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: Compositor.workspace(cell.number)
                }

                Row {
                    id: content
                    anchors.centerIn: parent
                    spacing: 3
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        text: cell.number
                        color: cell.isActive ? Theme.bg : cell.info.windows.length ? Theme.text : Theme.faint
                        font { family: Theme.mono; pointSize: Theme.pt(10.5); weight: Font.Medium }
                    }
                    Repeater {
                        model: cell.info.windows
                        delegate: Item {
                            id: icon
                            required property var modelData
                            anchors.verticalCenter: parent.verticalCenter
                            width: 13; height: 13
                            opacity: root.dragging?.id === modelData.id ? 0.35 : 1
                            Accessible.role: Accessible.Button
                            Accessible.name: modelData.title || modelData.app_id || ""
                            Accessible.onPressAction: Compositor.focusWindow(modelData.id)
                            AppIcon {
                                anchors.fill: parent
                                size: 13
                                entry: icon.modelData.app_id ? DesktopEntries.heuristicLookup(icon.modelData.app_id) : null
                                name: icon.modelData.app_id ?? ""
                            }
                            // Pressed and moved: dragged. Let go on another
                            // cell: the window moves there. A click: to it.
                            MouseArea {
                                anchors.fill: parent
                                anchors.margins: -2
                                preventStealing: true
                                cursorShape: root.dragging ? Qt.ClosedHandCursor : Qt.PointingHandCursor
                                property point from
                                property bool dragged: false
                                onPressed: m => { from = Qt.point(m.x, m.y); dragged = false }
                                onPositionChanged: m => {
                                    if (!pressed || (!root.dragging && Math.hypot(m.x - from.x, m.y - from.y) < 4)) return;
                                    const p = mapToItem(root, m.x, m.y);
                                    root.dragging = icon.modelData;
                                    root.dragAt = p;
                                    root.dropOn = root.cellAt(p.x);
                                }
                                onReleased: {
                                    const w = root.dragging, n = root.dropOn;
                                    if (!w) return;
                                    dragged = true;
                                    root.dragging = null;
                                    root.dropOn = 0;
                                    if (n > 0 && n !== w.workspace) Compositor.moveWindow(w.id, n);
                                }
                                onCanceled: { root.dragging = null; root.dropOn = 0 }
                                onClicked: if (!dragged) Compositor.focusWindow(icon.modelData.id)
                            }
                        }
                    }
                    Text {
                        anchors.verticalCenter: parent.verticalCenter
                        visible: cell.info.more > 0
                        text: "+" + cell.info.more
                        color: cell.isActive ? Theme.bg : Theme.muted
                        font { family: Theme.mono; pointSize: Theme.pt(9.5) }
                    }
                }
            }
        }
    }

    // The icon being dragged, under the pointer.
    Rectangle {
        visible: root.dragging !== null
        x: root.dragAt.x - width / 2
        y: root.dragAt.y - height / 2
        width: 22; height: 22; radius: 6
        color: Theme.surface2
        opacity: 0.9
        AppIcon {
            anchors.centerIn: parent
            size: 16
            entry: root.dragging?.app_id ? DesktopEntries.heuristicLookup(root.dragging.app_id) : null
            name: root.dragging?.app_id ?? ""
        }
    }

    // The workspace under the pointer, live, under its cell.
    Rectangle {
        id: preview
        readonly property Item over: root.previews && root.previewing > 0 && !root.dragging ? cells.itemAt(root.previewing - 1) : null
        readonly property var area: root.screen ? ({ x: root.screen.x, y: root.screen.y, width: root.screen.width, height: root.screen.height }) : null
        visible: over !== null && area !== null
        // Under the cell, but not off the screen's left edge.
        x: over ? Math.max(8 - root.mapToItem(null, 0, 0).x, over.mapToItem(root, over.width / 2, 0).x - width / 2) : 0
        y: root.height + (Theme.barHeight - root.height) / 2 + 6
        width: 272
        height: column.implicitHeight + 18
        radius: 12
        color: Theme.surface1
        border { width: 1; color: Theme.surface2 }
        Accessible.role: Accessible.StaticText
        Accessible.name: "Preview of workspace " + root.previewing

        Column {
            id: column
            x: 8; y: 8
            spacing: 8
            ClippingRectangle {
                width: 256
                height: preview.area ? Math.round(256 * preview.area.height / Math.max(1, preview.area.width)) : 144
                radius: 8
                color: Theme.bg
                Repeater {
                    model: preview.visible ? Strip.preview(Compositor.windows, root.previewing, preview.area, 256) : []
                    delegate: ClippingRectangle {
                        required property var modelData
                        x: modelData.x; y: modelData.y
                        width: modelData.width; height: modelData.height
                        radius: 3
                        color: Theme.surface2
                        ScreencopyView {
                            anchors.fill: parent
                            captureSource: preview.visible ? Compositor.toplevelFor(parent.modelData.id) : null
                            live: preview.visible
                            paintCursor: false
                            constraintSize: Qt.size(parent.width * 2, parent.height * 2)
                        }
                    }
                }
            }
            Column {
                x: 2
                spacing: 2
                Text {
                    text: "Workspace " + root.previewing
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(12); weight: Font.Medium }
                }
                Text {
                    width: 252
                    elide: Text.ElideRight
                    text: Strip.caption(Compositor.windows, root.previewing)
                    color: Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(11) }
                }
            }
        }
    }
}
