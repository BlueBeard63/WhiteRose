// Calendar (Figma: "Desktop - Calendar"): hangs from the right end of the bar,
// opened by clicking the clock. The time and date, the weather for the place
// chosen in the settings, a month you can page through with dots on days that have
// events, and the events of the selected day (today unless you click
// another). Events come from whiterosed (calendars in the settings database:
// iCal links kept in the keyring, and .ics files).
import QtQuick
import Quickshell
import Quickshell.Io
import "CalendarGrid.js" as Cal

SidePanel {
    id: root
    name: "calendar"
    bodyWidth: 320
    spacing: 0

    SystemClock { id: clock; precision: SystemClock.Minutes }

    // The month on show, and the day whose events are listed.
    property int year: clock.date.getFullYear()
    property int month: clock.date.getMonth()
    property date selected: clock.date
    function showToday() {
        const t = new Date();
        year = t.getFullYear(); month = t.getMonth(); selected = t;
    }
    // Paging selects today in today's month, else the 1st (the events
    // loaded are the new month's, so the old selection would show none).
    function page(delta) {
        const d = new Date(year, month + delta, 1);
        const t = new Date();
        year = d.getFullYear(); month = d.getMonth();
        selected = year === t.getFullYear() && month === t.getMonth() ? t : d;
    }
    onOpenChanged: if (open) { showToday(); load() }

    // ---- events ------------------------------------------------------------
    property var events: []
    readonly property var busy: Cal.busyDays(events)
    readonly property var dayEvents: Cal.onDay(events, selected)

    function load() {
        const r = Cal.gridRange(year, month);
        const want = r.from;
        Daemon.request({ op: "calendar_events", from: r.from, to: r.to }, (events, err) => {
            if (err) { console.warn("calendar events:", err); return; }
            // A reply for a month we've already paged away from is dropped.
            if (Cal.gridRange(root.year, root.month).from === want) root.events = events ?? [];
        });
    }
    onYearChanged: if (open) load()
    onMonthChanged: if (open) load()
    Connections {
        target: System
        function onCalendarsChanged() { if (root.open) root.load() }
    }

    // ---- time, date, weather ------------------------------------------------
    Item {
        width: parent.width
        height: 72
        Column {
            y: 2
            spacing: 2
            Text {
                text: Qt.formatTime(clock.date, "HH:mm")
                color: Theme.text
                font { family: Theme.mono; pointSize: Theme.pt(32); weight: Font.Medium }
            }
            Text {
                text: Qt.formatDate(clock.date, "dddd, d MMMM")
                color: Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(12.5) }
            }
        }
        Rectangle {
            visible: System.weather !== null
            anchors { right: parent.right; top: parent.top; topMargin: 12 }
            height: 28
            width: weatherRow.implicitWidth + 20
            radius: 14
            color: Theme.surface1
            Row {
                id: weatherRow
                anchors.centerIn: parent
                spacing: 6
                Icon { anchors.verticalCenter: parent.verticalCenter; name: System.weather?.icon ?? "cloud"; size: 15; color: Theme.rose }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    text: (System.weather?.temperature ?? "") + "°"
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(12.5); weight: Font.Medium }
                }
            }
            HoverHandler { id: weatherHover }
            // Where and what, on hover.
            Text {
                visible: weatherHover.hovered
                anchors { right: parent.right; top: parent.bottom; topMargin: 4 }
                text: (System.weather?.description ?? "") + " · " + (System.weather?.place ?? "")
                color: Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(11) }
            }
        }
    }

    // ---- month ---------------------------------------------------------------
    Item {
        width: parent.width
        height: 36
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: Qt.formatDate(new Date(root.year, root.month, 1), "MMMM yyyy")
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
            MouseArea {
                anchors.fill: parent
                cursorShape: Qt.PointingHandCursor
                onClicked: root.showToday()
            }
        }
        Row {
            anchors { right: parent.right; verticalCenter: parent.verticalCenter }
            spacing: 2
            IconButton { width: 26; height: 26; icon: "chevL"; iconSize: 14; fill: "transparent"; iconColor: Theme.muted; onClicked: root.page(-1) }
            IconButton { width: 26; height: 26; icon: "chevR"; iconSize: 14; fill: "transparent"; iconColor: Theme.muted; onClicked: root.page(1) }
        }
    }

    Grid {
        id: grid
        columns: 7
        readonly property real cell: parent.width / 7
        Repeater {
            model: ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"]
            delegate: Text {
                required property string modelData
                width: grid.cell; height: 24
                horizontalAlignment: Text.AlignHCenter
                verticalAlignment: Text.AlignVCenter
                text: modelData
                color: Theme.faint
                font { family: Theme.font; pointSize: Theme.pt(11) }
            }
        }
        Repeater {
            model: Cal.monthGrid(root.year, root.month, clock.date)
            delegate: Item {
                id: dayCell
                required property var modelData
                readonly property bool isSelected: Cal.key(modelData.date) === Cal.key(root.selected)
                width: grid.cell; height: 36
                Rectangle {
                    anchors.centerIn: parent
                    width: 28; height: 28; radius: 14
                    color: dayCell.modelData.today ? Theme.pearl
                         : dayCell.isSelected ? Theme.surface2
                         : dayMouse.containsMouse ? Theme.surface1 : "transparent"
                }
                Text {
                    anchors.centerIn: parent
                    text: dayCell.modelData.day
                    color: dayCell.modelData.today ? Theme.bg
                         : !dayCell.modelData.inMonth ? Theme.faint
                         : dayCell.modelData.weekend ? Theme.muted : Theme.text
                    font {
                        family: Theme.font; pointSize: Theme.pt(12.5)
                        weight: dayCell.modelData.today ? Font.DemiBold : Font.Normal
                    }
                }
                // A dot under days with events.
                Rectangle {
                    visible: root.busy[Cal.key(dayCell.modelData.date)] === true
                    anchors { horizontalCenter: parent.horizontalCenter; bottom: parent.bottom; bottomMargin: 1 }
                    width: 4; height: 4; radius: 2
                    color: Theme.rose
                }
                MouseArea {
                    id: dayMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: {
                        const d = dayCell.modelData.date;
                        // A greyed day from the next or previous month opens that month.
                        if (!dayCell.modelData.inMonth) root.page(d < new Date(root.year, root.month, 1) ? -1 : 1);
                        root.selected = d;
                    }
                }
            }
        }
    }

    // ---- the selected day's events ----------------------------------------------
    Item { width: parent.width; height: 14 }
    Rectangle { width: parent.width; height: 1; color: Theme.surface1 }
    Item { width: parent.width; height: 14 }
    Text {
        text: Cal.dayLabel(root.selected, clock.date)
        color: Theme.faint
        font { family: Theme.font; pointSize: Theme.pt(11); letterSpacing: 0.6 }
    }
    Item { width: parent.width; height: 10 }
    Column {
        width: parent.width
        spacing: 10
        Repeater {
            model: root.dayEvents
            delegate: Row {
                required property var modelData
                width: parent.width
                spacing: 10
                Rectangle {
                    width: 3; height: info.implicitHeight; radius: 1.5
                    color: modelData.color
                }
                Column {
                    id: info
                    width: parent.width - 13
                    spacing: 2
                    Text {
                        width: parent.width
                        text: modelData.title
                        elide: Text.ElideRight
                        color: Theme.text
                        font { family: Theme.font; pointSize: Theme.pt(13) }
                    }
                    Text {
                        width: parent.width
                        text: Cal.timeRange(modelData, root.selected) + (modelData.location ? " · " + modelData.location.split("\n")[0] : "")
                        elide: Text.ElideRight
                        color: Theme.muted
                        font { family: Theme.mono; pointSize: Theme.pt(11) }
                    }
                }
            }
        }
        Text {
            visible: root.dayEvents.length === 0
            text: "No events"
            color: Theme.faint
            font { family: Theme.font; pointSize: Theme.pt(12.5) }
        }
    }
}
