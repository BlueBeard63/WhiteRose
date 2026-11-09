// The screenshot tool on screen (Figma: "Screenshot - Region (SUPER + SHIFT
// + S)"): the toolbar in the island's place, and on every screen a dim with
// the selection cut out. Region: drag to select, drag inside to move, the
// handles resize, Shift keeps it square. Window: point at a window and
// click. Enter saves, Ctrl+C copies, Esc cancels; double-click saves.
import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.settings
import "Shot.js" as Shot

Variants {
    model: ScreenshotTool.open ? Quickshell.screens : []
    delegate: PanelWindow {
        id: win
        required property var modelData
        screen: modelData
        anchors { top: true; bottom: true; left: true; right: true }
        exclusionMode: ExclusionMode.Ignore
        color: "transparent"
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.namespace: "whiterose-screenshot"
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive

        // This screen's place in the shared (global) space.
        readonly property real ox: modelData.x
        readonly property real oy: modelData.y
        readonly property real scale: Compositor.outputs.find(o => o.connector === modelData.name)?.scale ?? 1
        // The toolbar goes on the main screen (built-in, else the first).
        readonly property bool main: (Compositor.outputs.find(o => o.enabled)?.connector ?? Quickshell.screens[0]?.name) === modelData.name

        // What's selected, in this screen's coordinates (null: nothing here).
        readonly property var sel: {
            const t = ScreenshotTool;
            const r = t.mode === "region" ? t.region
                : t.mode === "window" && t.window ? { x: t.window.geometry[0], y: t.window.geometry[1], w: t.window.geometry[2], h: t.window.geometry[3] }
                : t.mode === "screen" ? { x: win.ox, y: win.oy, w: win.width, h: win.height } : null;
            return r ? { x: r.x - win.ox, y: r.y - win.oy, w: r.w, h: r.h } : null;
        }

        // ---- dim, with the selection cut out (the bar stays as it is) -----------
        readonly property real top: Theme.barHeight
        Item {
            anchors.fill: parent
            readonly property var s: win.sel ?? { x: 0, y: win.top, w: 0, h: 0 }
            readonly property real sx: Math.max(0, s.x)
            readonly property real sy: Math.max(win.top, s.y)
            readonly property real ex: Math.min(win.width, s.x + s.w)
            readonly property real ey: Math.min(win.height, s.y + s.h)
            readonly property color dim: Qt.rgba(0, 0, 0, 0.5)
            Rectangle { x: 0; y: win.top; width: parent.width; height: Math.max(0, parent.sy - win.top); color: parent.dim }
            Rectangle { x: 0; y: parent.ey; width: parent.width; height: Math.max(0, win.height - parent.ey); color: parent.dim }
            Rectangle { x: 0; y: parent.sy; width: parent.sx; height: Math.max(0, parent.ey - parent.sy); color: parent.dim }
            Rectangle { x: parent.ex; y: parent.sy; width: Math.max(0, win.width - parent.ex); height: Math.max(0, parent.ey - parent.sy); color: parent.dim }
            // No selection on this screen: all of it dimmed.
            Rectangle { visible: win.sel === null || parent.ex <= parent.sx; x: 0; y: win.top; width: parent.width; height: win.height - win.top; color: parent.dim }
        }

        // ---- the selection --------------------------------------------------------
        Rectangle {
            id: frame
            visible: win.sel !== null && ScreenshotTool.mode !== "screen"
            x: win.sel?.x ?? 0; y: win.sel?.y ?? 0
            width: win.sel?.w ?? 0; height: win.sel?.h ?? 0
            color: "transparent"
            border { width: 1.5; color: "white" }
            // Handles (region mode).
            Repeater {
                model: ScreenshotTool.mode === "region" ? ["nw", "n", "ne", "e", "se", "s", "sw", "w"] : []
                delegate: Rectangle {
                    required property string modelData
                    width: 9; height: 9; radius: 2
                    color: "white"
                    x: (modelData.includes("w") ? 0 : modelData.includes("e") ? frame.width : frame.width / 2) - 4.5
                    y: (modelData.includes("n") ? 0 : modelData.includes("s") ? frame.height : frame.height / 2) - 4.5
                }
            }
        }
        // Its size in real pixels.
        Rectangle {
            visible: frame.visible
            x: frame.x
            y: frame.y + frame.height + 10 + height > win.height ? frame.y - height - 10 : frame.y + frame.height + 10
            width: sizeRow.implicitWidth + 16; height: 23; radius: 7
            color: Qt.rgba(0, 0, 0, 0.75)
            Row {
                id: sizeRow
                anchors.centerIn: parent
                spacing: 6
                Text { text: frame.visible ? Shot.sizeLabel(win.sel, win.scale) : ""; color: "white"; font { family: Theme.mono; pointSize: Theme.pt(11.5) } }
                Text { text: "px"; color: "#A0A0A8"; font { family: Theme.mono; pointSize: Theme.pt(10.5) } }
            }
        }
        // Screen mode: the whole screen, outlined.
        Rectangle {
            visible: ScreenshotTool.mode === "screen"
            anchors.fill: parent
            anchors.topMargin: win.top
            color: "transparent"
            border { width: 2; color: "white" }
        }

        // ---- the mouse ----------------------------------------------------------------
        MouseArea {
            anchors.fill: parent
            anchors.topMargin: win.top
            hoverEnabled: true
            cursorShape: ScreenshotTool.mode === "region" ? (drag === "move" ? Qt.ClosedHandCursor : Qt.CrossCursor) : Qt.PointingHandCursor
            property string drag: ""           // new | move | n, ne, ...
            property point start
            property var startRect: null
            readonly property real gx: mouseX + win.ox
            readonly property real gy: mouseY + win.top + win.oy

            function handleAt(x, y) {
                const r = ScreenshotTool.region;
                if (!r) return "";
                const near = (a, b) => Math.abs(a - b) <= 8;
                const v = near(y, r.y) ? "n" : near(y, r.y + r.h) ? "s" : "";
                const h = near(x, r.x) ? "w" : near(x, r.x + r.w) ? "e" : "";
                const inX = x >= r.x - 8 && x <= r.x + r.w + 8, inY = y >= r.y - 8 && y <= r.y + r.h + 8;
                if ((v && inX) || (h && inY)) return v + h;
                return x > r.x && x < r.x + r.w && y > r.y && y < r.y + r.h ? "move" : "";
            }
            onPressed: m => {
                if (ScreenshotTool.mode !== "region") return;
                start = Qt.point(gx, gy);
                startRect = ScreenshotTool.region;
                drag = handleAt(gx, gy) || "new";
            }
            onPositionChanged: m => {
                const t = ScreenshotTool;
                if (t.mode === "window") {
                    const w = Shot.windowAt(Compositor.windows, gx, gy, Compositor.activeWorkspace);
                    if ((w?.id ?? -1) !== (t.window?.id ?? -1)) t.window = w;
                    return;
                }
                if (!pressed || t.mode !== "region") return;
                const square = (m.modifiers & Qt.ShiftModifier) !== 0;
                const bounds = { x: win.ox, y: win.oy + win.top, w: win.width, h: win.height - win.top };
                if (drag === "new") t.region = Shot.fromDrag(start, Qt.point(gx, gy), square);
                else if (drag === "move") t.region = Shot.clampMove({ x: startRect.x + gx - start.x, y: startRect.y + gy - start.y, w: startRect.w, h: startRect.h }, bounds);
                else t.region = Shot.resize(startRect, drag, gx - start.x, gy - start.y, square);
            }
            onReleased: drag = ""
            onClicked: m => { if (ScreenshotTool.mode === "screen") ScreenshotTool.capture(true) }
            onDoubleClicked: ScreenshotTool.capture(true)
        }

        // ---- keys -------------------------------------------------------------------------
        Item {
            focus: true
            Keys.onPressed: e => {
                if (e.key === Qt.Key_Escape) ScreenshotTool.cancel();
                else if (e.key === Qt.Key_Return || e.key === Qt.Key_Enter) ScreenshotTool.capture(true);
                else if (e.key === Qt.Key_C && (e.modifiers & Qt.ControlModifier)) ScreenshotTool.capture(false);
                else return;
                e.accepted = true;
            }
        }

        // ---- the toolbar, where the island is ----------------------------------------------
        Item {
            visible: win.main
            anchors { top: parent.top; topMargin: win.top; horizontalCenter: parent.horizontalCenter }
            width: shape.width; height: shape.height
            // Clicks on the toolbar's background don't start a selection.
            MouseArea { anchors.fill: parent; acceptedButtons: Qt.AllButtons }
            PanelShape {
                id: shape
                bodyWidth: tools.implicitWidth + 32
                bodyHeight: 56
                earWidth: 26; earHeight: 30
                bottomLeftRadius: 26; bottomRightRadius: 26
            }
            Row {
                id: tools
                x: shape.bodyX + 16
                y: 12
                spacing: 10

                // Region / Window / Screen.
                Rectangle {
                    width: modes.implicitWidth + 4; height: 32; radius: 10
                    color: Theme.surface1
                    Row {
                        id: modes
                        anchors.centerIn: parent
                        spacing: 2
                        Repeater {
                            model: [["region", "Region", "squareDashed"], ["window", "Window", "appWindow"], ["screen", "Screen", "monitor"]]
                            delegate: Rectangle {
                                required property var modelData
                                readonly property bool on: ScreenshotTool.mode === modelData[0]
                                width: modeRow.implicitWidth + 22; height: 28; radius: 8
                                color: on ? Theme.pearl : modeMouse.containsMouse ? Theme.surface2 : "transparent"
                                Row {
                                    id: modeRow
                                    anchors.centerIn: parent
                                    spacing: 6
                                    Icon { anchors.verticalCenter: parent.verticalCenter; name: modelData[2]; size: 14; color: parent.parent.on ? Theme.bg : Theme.text }
                                    Text { anchors.verticalCenter: parent.verticalCenter; text: modelData[1]; color: parent.parent.on ? Theme.bg : Theme.text; font { family: Theme.font; pointSize: Theme.pt(12); weight: Font.Medium } }
                                }
                                MouseArea { id: modeMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: ScreenshotTool.mode = modelData[0] }
                            }
                        }
                    }
                }
                Rectangle { anchors.verticalCenter: parent.verticalCenter; width: 1; height: 22; color: Theme.surface2 }
                Choice {
                    value: ScreenshotTool.delay
                    options: [{ label: "No delay", value: 0 }, { label: "3 s delay", value: 3 }, { label: "5 s delay", value: 5 }, { label: "10 s delay", value: 10 }]
                    onPicked: v => ScreenshotTool.delay = v
                }
                // Include the pointer (where the compositor draws it).
                Rectangle {
                    anchors.verticalCenter: parent.verticalCenter
                    width: ptrRow.implicitWidth + 20; height: 30; radius: 9
                    opacity: ScreenshotTool.canPointer ? 1 : 0.4
                    color: ScreenshotTool.pointer && ScreenshotTool.canPointer ? Theme.pearl : ptrMouse.containsMouse ? Theme.grip : Theme.surface2
                    Row {
                        id: ptrRow
                        anchors.centerIn: parent
                        spacing: 6
                        readonly property color ink: ScreenshotTool.pointer && ScreenshotTool.canPointer ? Theme.bg : Theme.text
                        Icon { anchors.verticalCenter: parent.verticalCenter; name: "pointer"; size: 14; color: parent.ink }
                        Text { anchors.verticalCenter: parent.verticalCenter; text: "Pointer"; color: parent.ink; font { family: Theme.font; pointSize: Theme.pt(12); weight: Font.Medium } }
                    }
                    MouseArea { id: ptrMouse; anchors.fill: parent; enabled: ScreenshotTool.canPointer; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: ScreenshotTool.pointer = !ScreenshotTool.pointer }
                }
                Item { width: 14; height: 1 }
                Button { text: "Copy"; icon: "copy"; enabled: ScreenshotTool.ready; onClicked: ScreenshotTool.capture(false) }
                Button { text: "Save"; icon: "camera"; kind: "primary"; enabled: ScreenshotTool.ready; onClicked: ScreenshotTool.capture(true) }
            }
        }
    }
}
