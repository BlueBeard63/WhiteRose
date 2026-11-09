// Date and time (Figma: Trigger/Clock). SystemClock with minute precision
// wakes once a minute, not every second.
import QtQuick
import Quickshell

Trigger {
    id: root
    spacing: 7
    SystemClock { id: clock; precision: SystemClock.Minutes }

    Text {
        anchors.verticalCenter: parent.verticalCenter
        text: Qt.formatDateTime(clock.date, "ddd d MMM")
        color: Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(11.5) }
    }
    Text {
        anchors.verticalCenter: parent.verticalCenter
        text: Qt.formatDateTime(clock.date, "hh:mm")
        color: Theme.text
        font { family: Theme.mono; pointSize: Theme.pt(11.5); weight: Font.Medium }
    }
}
