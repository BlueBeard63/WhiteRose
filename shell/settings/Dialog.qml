// Asked in place, never a window over Settings. Two kinds:
//   strip: true   a question: a strip across the top of the page (its
//                 title and subtitle, anything in it beside them, the
//                 footer's buttons at the right).
//   strip: false  something to fill in, or a thing's own settings: its own
//                 page over this one, with ‹ back to the page it came from,
//                 the body scrolling and the footer pinned at the bottom.
// Same use as before: title, subtitle, icon, children, footer, open(),
// close(). Esc closes either; going to another page closes it too.
import QtQuick
import QtQuick.Controls
import qs

Item {
    id: root
    property string title
    property string subtitle
    property string icon
    property bool strip: false
    default property alias content: body.data
    property alias footer: foot.data
    // (Ignored: it fits the page.)
    property real dialogWidth: 0
    // (Like a Popup's NoAutoClose: Esc and ‹ back do nothing while busy;
    // its own buttons still close it.)
    property bool canClose: true
    signal opened()
    signal closed()
    // Open (shown in its place at the top of the page, or as its own page).
    property bool isOpen: false
    function open() {
        // One at a time.
        if (SettingsState.asking && SettingsState.asking !== root) SettingsState.asking.close();
        SettingsState.asking = root;
        root.isOpen = true;
        root.opened();
        Qt.callLater(() => shown.forceActiveFocus());
    }
    function close() {
        if (!root.isOpen) return;
        root.isOpen = false;
        if (SettingsState.asking === root) SettingsState.asking = null;
        root.closed();
    }
    function dismiss() { if (root.canClose) root.close() }
    Keys.onEscapePressed: root.dismiss()
    Connections { target: SettingsState; function onPageChanged() { root.close() } function onSubChanged() { root.close() } }

    FocusScope {
        id: shown
        parent: root.strip ? SettingsState.stripHost : SettingsState.sheetHost
        visible: root.isOpen
        width: parent ? parent.width : 0
        height: root.strip ? stripBox.height + 8 : (parent ? parent.height : 0)
        Keys.onEscapePressed: root.dismiss()
        // ---- a strip ----
        Rectangle {
            id: stripBox
            visible: root.strip
            width: parent.width
            height: Math.max(52, Math.max(stripText.implicitHeight, body.implicitHeight, foot.implicitHeight) + 20)
            radius: 12
            color: Qt.rgba(0.89, 0.70, 0.25, 0.08)
            border { width: 1; color: Qt.rgba(0.89, 0.70, 0.25, 0.45) }
            Icon {
                id: stripIcon
                x: 14; anchors.verticalCenter: parent.verticalCenter
                name: root.icon || "alert"; size: 15; color: "#E3B341"
            }
            Column {
                id: stripText
                anchors { left: stripIcon.right; leftMargin: 10; verticalCenter: parent.verticalCenter }
                width: Math.min(Math.max(stTitle.implicitWidth, stSub.implicitWidth), parent.width * 0.5)
                spacing: 1
                Text { id: stTitle; width: parent.width; elide: Text.ElideRight; text: root.title; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(12.5); weight: Font.Medium } }
                Text { id: stSub; visible: text !== ""; width: parent.width; wrapMode: Text.Wrap; text: root.subtitle; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
            }
        }

        // ---- its own page ----
        Rectangle {
            id: sheet
            visible: !root.strip
            anchors.fill: parent
            color: Theme.bg
            // ‹ back
            Row {
                id: back
                x: 28; y: 2
                spacing: 6
                Icon { anchors.verticalCenter: parent.verticalCenter; name: "chevL"; size: 13; color: backMouse.containsMouse ? Theme.text : Theme.muted }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: SettingsState.currentSub?.name ?? SettingsState.current?.name ?? "Back"
                    color: backMouse.containsMouse ? Theme.text : Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(12.5) }
                }
            }
            MouseArea { id: backMouse; x: back.x - 6; y: back.y - 6; width: back.width + 12; height: back.height + 12; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: root.dismiss() }
            Row {
                id: head
                x: 28; y: back.y + back.height + 18
                spacing: 12
                Rectangle {
                    visible: root.icon !== ""
                    width: 38; height: 38; radius: 11
                    color: Theme.rose
                    Icon { anchors.centerIn: parent; name: root.icon; size: 18; color: Theme.onAccent }
                }
                Column {
                    anchors.verticalCenter: parent.verticalCenter
                    spacing: 2
                    Text { text: root.title; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(17); weight: Font.DemiBold } }
                    Text { visible: text !== ""; text: root.subtitle; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
                }
            }
            Flickable {
                id: scroller
                anchors { left: parent.left; right: parent.right; top: head.bottom; topMargin: 18; bottom: footBar.visible ? footBar.top : parent.bottom }
                contentHeight: bodyHolder.height + 24
                clip: true
                boundsBehavior: Flickable.StopAtBounds
                ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
                Item {
                    id: bodyHolder
                    x: 28; width: Math.min(scroller.width - 56, 720)
                    height: root.strip ? 0 : body.implicitHeight
                }
            }
            Rectangle {
                id: footBar
                visible: !root.strip && foot.children.length > 0
                anchors { left: parent.left; right: parent.right; bottom: parent.bottom }
                height: 56
                color: Theme.bg
                Rectangle { width: parent.width; height: 1; color: Theme.surface1 }
            }
        }
    }

    // The body and the footer: in the page, or in the strip.
    Column {
        id: body
        parent: root.strip ? stripBox : bodyHolder
        x: root.strip ? stripText.x + stripText.width + 16 : 0
        anchors.verticalCenter: root.strip ? parent?.verticalCenter : undefined
        width: root.strip ? Math.max(0, stripBox.width - x - foot.width - 24) : bodyHolder.width
        spacing: 12
    }
    Row {
        id: foot
        parent: root.strip ? stripBox : footBar
        anchors { right: parent?.right; rightMargin: root.strip ? 8 : 28; verticalCenter: parent?.verticalCenter }
        spacing: 8
    }
}
