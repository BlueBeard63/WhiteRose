// A WhiteRose app's window: rounded, in the desktop's colours, with the
// desktop's title buttons (minimise, maximise / restore, close) at the top
// right. Drag the title bar to move it (double-click maximises), drag the
// edges to resize. Put toolbar items in `toolbar` (left of the buttons) and
// the app below.
import QtQuick
import QtQuick.Window

Window {
    id: win
    property string titleText: title
    // Things in the title bar, after the title (tabs, a path, buttons).
    default property alias content: body.data
    property alias toolbar: toolbarRow.data
    property alias titleBar: bar
    readonly property bool maximized: visibility === Window.Maximized || visibility === Window.FullScreen
    flags: Qt.Window | Qt.FramelessWindowHint
    color: "transparent"
    width: 900; height: 620
    minimumWidth: 420; minimumHeight: 300

    function toggleMaximized() { if (win.maximized) win.showNormal(); else win.showMaximized() }

    Rectangle {
        id: frame
        anchors.fill: parent
        radius: win.maximized ? 0 : 12
        color: Theme.bg
        border { width: win.maximized ? 0 : 1; color: Theme.surface2 }

        // ---- title bar ---------------------------------------------------------
        Item {
            id: bar
            anchors { left: parent.left; right: parent.right; top: parent.top }
            height: 42
            MouseArea {
                anchors.fill: parent
                acceptedButtons: Qt.LeftButton
                onPressed: win.startSystemMove()
                onDoubleClicked: win.toggleMaximized()
            }
            Row {
                id: toolbarRow
                anchors { left: parent.left; leftMargin: 14; right: buttons.left; rightMargin: 10; verticalCenter: parent.verticalCenter }
                spacing: 8
            }
            Row {
                id: buttons
                anchors { top: parent.top; right: parent.right }
                Repeater {
                    model: [
                        { icon: "minus", run: () => win.showMinimized() },
                        { icon: win.maximized ? "restore" : "square", run: () => win.toggleMaximized() },
                        { icon: "x", close: true, run: () => win.close() },
                    ]
                    delegate: Rectangle {
                        required property var modelData
                        width: 46; height: 34
                        color: !btn.containsMouse ? "transparent" : modelData.close ? "#C42B1C" : Theme.surface2
                        Icon {
                            anchors.centerIn: parent
                            name: modelData.icon
                            size: modelData.icon === "x" ? 14 : 12
                            color: btn.containsMouse && modelData.close ? "white" : Theme.text
                        }
                        MouseArea { id: btn; anchors.fill: parent; hoverEnabled: true; onClicked: modelData.run() }
                    }
                }
            }
        }

        // ---- the app -----------------------------------------------------------------
        Item {
            id: body
            anchors { left: parent.left; right: parent.right; top: bar.bottom; bottom: parent.bottom; margins: win.maximized ? 0 : 1 }
        }
    }

    // ---- resizing from the edges (not while maximised) ---------------------------------
    component Edge: MouseArea {
        property int edges
        enabled: !win.maximized
        cursorShape: edges === (Qt.LeftEdge | Qt.TopEdge) || edges === (Qt.RightEdge | Qt.BottomEdge) ? Qt.SizeFDiagCursor
            : edges === (Qt.RightEdge | Qt.TopEdge) || edges === (Qt.LeftEdge | Qt.BottomEdge) ? Qt.SizeBDiagCursor
            : edges & (Qt.LeftEdge | Qt.RightEdge) ? Qt.SizeHorCursor : Qt.SizeVerCursor
        onPressed: win.startSystemResize(edges)
    }
    readonly property int grab: 6
    Edge { edges: Qt.LeftEdge; anchors { left: parent.left; top: parent.top; bottom: parent.bottom; topMargin: 12; bottomMargin: 12 } width: win.grab }
    Edge { edges: Qt.RightEdge; anchors { right: parent.right; top: parent.top; bottom: parent.bottom; topMargin: 12; bottomMargin: 12 } width: win.grab }
    Edge { edges: Qt.BottomEdge; anchors { left: parent.left; right: parent.right; bottom: parent.bottom; leftMargin: 12; rightMargin: 12 } height: win.grab }
    Edge { edges: Qt.TopEdge; anchors { left: parent.left; right: parent.right; top: parent.top; leftMargin: 12; rightMargin: 150 } height: 4 }
    Edge { edges: Qt.LeftEdge | Qt.BottomEdge; anchors { left: parent.left; bottom: parent.bottom } width: 12; height: 12 }
    Edge { edges: Qt.RightEdge | Qt.BottomEdge; anchors { right: parent.right; bottom: parent.bottom } width: 12; height: 12 }
    Edge { edges: Qt.LeftEdge | Qt.TopEdge; anchors { left: parent.left; top: parent.top } width: 12; height: 12 }
}
