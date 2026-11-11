// A question asked in place: a strip right above the files (not a window
// over them). What's asked goes in it (a text, a box, a switch…), the
// buttons at its right. Esc closes it; shown with open(), gone with close().
//   tone: plain | danger (deleting, running something untrusted) | warn
import QtQuick
import WhiteRose.Kit

FocusScope {
    id: root
    default property alias content: what.data
    property alias buttons: right.data
    property string icon: ""
    property string tone: "plain"
    property bool asking: false
    // How wide what's asked can be (beside the buttons).
    readonly property real room: what.width
    // Something in it to focus when it opens (a Field, a Button).
    property var focusOn: null
    signal opened()
    signal closed()
    function open() { root.asking = true; root.opened(); Qt.callLater(() => { if (root.focusOn) { if (root.focusOn.focusField) root.focusOn.focusField(); else root.focusOn.forceActiveFocus() } else root.forceActiveFocus() }) }
    function close() { if (!root.asking) return; root.asking = false; root.closed() }

    visible: asking
    height: asking ? Math.max(48, what.implicitHeight + 20) : 0
    Keys.onEscapePressed: root.close()

    Rectangle {
        anchors { fill: parent; leftMargin: 12; rightMargin: 12; topMargin: 4; bottomMargin: 4 }
        radius: 10
        color: root.tone === "danger" ? Qt.rgba(0.9, 0.33, 0.29, 0.12) : root.tone === "warn" ? Qt.rgba(0.89, 0.70, 0.25, 0.08) : Theme.surface1
        border { width: 1; color: root.tone === "danger" ? Qt.rgba(0.9, 0.33, 0.29, 0.45) : root.tone === "warn" ? Qt.rgba(0.89, 0.70, 0.25, 0.45) : Theme.surface2 }
        Icon {
            id: ic
            visible: root.icon !== ""
            x: 12; anchors.verticalCenter: parent.verticalCenter
            name: root.icon; size: 14
            color: root.tone === "danger" ? Theme.rose : root.tone === "warn" ? "#E3B341" : Theme.muted
        }
        Row {
            id: what
            anchors { left: root.icon !== "" ? ic.right : parent.left; leftMargin: root.icon !== "" ? 10 : 14; right: right.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
            spacing: 10
        }
        Row {
            id: right
            anchors { right: parent.right; rightMargin: 6; verticalCenter: parent.verticalCenter }
            spacing: 4
        }
    }
}
