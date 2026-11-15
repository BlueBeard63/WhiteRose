// Date and time (Figma: Trigger/Clock); click for the calendar. SystemClock
// with minute precision wakes once a minute; only showing seconds (Settings >
// Top bar) makes it tick every second.
import QtQuick
import Quickshell

Trigger {
    id: root
    spacing: 7
    readonly property bool twelve: Daemon.setting("bar.timeFormat", "24h") === "12h"
    readonly property bool seconds: Daemon.setting("bar.seconds", false)
    shows: (date.visible ? date.text + " " : "") + time.text
    SystemClock { id: clock; precision: root.seconds ? SystemClock.Seconds : SystemClock.Minutes }

    Text {
        id: date
        anchors.verticalCenter: parent.verticalCenter
        visible: Daemon.setting("bar.date", true)
        text: Qt.formatDateTime(clock.date, "ddd d MMM")
        color: Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(11.5) }
    }
    Text {
        id: time
        anchors.verticalCenter: parent.verticalCenter
        text: Qt.formatDateTime(clock.date, (root.twelve ? "h:mm" : "hh:mm") + (root.seconds ? ":ss" : "") + (root.twelve ? " AP" : ""))
        color: Theme.text
        font { family: Theme.mono; pointSize: Theme.pt(11.5); weight: Font.Medium }
    }
}
