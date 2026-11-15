// Island view: the agenda (Figma: "Island v2 — Agenda"; the Agenda module,
// the clock or SUPER+A). Today's next event (or the one on now), how long
// until it, and the day as a line: its events on it, the next one lit, now
// in rose. Clicking the line or the time opens the month (the calendar
// panel). Updates each minute while it's open.
import QtQuick
import qs
import "Agenda.js" as Ag

Item {
    id: root
    implicitWidth: 560
    implicitHeight: 18 + top.height + 22 + line.height + 16

    property var events: []
    property date now: new Date()
    readonly property var next: Ag.next(root.events, root.now)
    readonly property var span: Ag.span(root.events, root.now)
    readonly property var blocks: Ag.blocks(root.events, root.now, root.span, root.next?.event ?? null)
    readonly property int w: root.implicitWidth - 48

    function pad(n) { return n < 10 ? "0" + n : "" + n }
    function day(d) { return d.getFullYear() + "-" + pad(d.getMonth() + 1) + "-" + pad(d.getDate()) }
    function load() {
        const t = new Date(), tomorrow = new Date(t.getFullYear(), t.getMonth(), t.getDate() + 1);
        Daemon.request({ op: "calendar_events", from: day(t), to: day(tomorrow) }, (events, err) => {
            if (err) { console.warn("agenda:", err); return; }
            root.events = events ?? [];
        });
    }
    function month() { ShellState.close(); ShellState.togglePanel("calendar") }
    Component.onCompleted: { load(); keys.forceActiveFocus() }
    Timer { interval: 60000; repeat: true; running: true; onTriggered: root.now = new Date() }
    Connections { target: System; function onCalendarsChanged() { root.load() } }
    Item { id: keys; focus: true; Keys.onEscapePressed: ShellState.close() }

    // ---- what's next ------------------------------------------------------------------
    Item {
        id: top
        x: 24; y: 18
        width: root.w
        height: 50
        Text {
            width: parent.width - time.width - 16
            elide: Text.ElideRight
            text: root.next ? root.next.event.title : "Nothing else today"
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(16); weight: Font.DemiBold }
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }
        Text {
            y: 30
            width: parent.width - time.width - 16
            elide: Text.ElideRight
            text: root.next ? [Ag.when(root.next), root.next.event.location || root.next.event.calendar].filter(s => s).join(" · ")
                : (root.blocks.length > 0 ? root.blocks.length + (root.blocks.length === 1 ? " event" : " events") + " earlier" : "")
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(10.5) }
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }
        Text {
            id: time
            anchors.right: parent.right
            visible: !!root.next
            text: root.next ? Ag.hm(new Date(root.next.event.start)) : ""
            color: Theme.text
            font { family: Theme.mono; pointSize: Theme.pt(22); weight: Font.Medium }
            MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: root.month() }
        }
    }

    // ---- the day ----------------------------------------------------------------------
    Item {
        id: line
        x: 24; y: top.y + top.height + 22
        width: root.w
        height: 34
        Accessible.role: Accessible.Button
        Accessible.name: "Month"
        Accessible.onPressAction: root.month()
        Rectangle { y: 4; width: parent.width; height: 2; radius: 1; color: Theme.surface1 }
        Repeater {
            model: root.blocks
            delegate: Rectangle {
                required property var modelData
                x: modelData.x0 * line.width
                width: Math.max(6, (modelData.x1 - modelData.x0) * line.width)
                height: 10; radius: 5
                color: modelData.next ? Theme.pearl : modelData.past ? Theme.surface1 : Theme.surface2
            }
        }
        Rectangle {
            x: Ag.at(root.now, root.now, root.span) * line.width - 1
            y: -4
            width: 2; height: 18; radius: 1
            color: Theme.rose
        }
        Text {
            y: 20
            text: root.pad(root.span.from) + ":00"
            color: Theme.faint
            font { family: Theme.mono; pointSize: Theme.pt(8.5) }
        }
        Text {
            y: 20
            anchors.right: parent.right
            text: root.pad(root.span.to % 24) + ":00"
            color: Theme.faint
            font { family: Theme.mono; pointSize: Theme.pt(8.5) }
        }
        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: root.month() }
    }
}
