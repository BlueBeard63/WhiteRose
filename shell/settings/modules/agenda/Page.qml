// Agenda: today's next event and the day as a line, in the island, from the
// clock or SUPER+A (the month is a click away). Turned off in Settings ›
// Modules, the clock opens the month as before.
import QtQuick
import qs
import qs.settings

Column {
    id: page
    spacing: 16

    PageHeader { title: "Agenda"; subtitle: "Today's next event and the day, from the clock or SUPER+A. Click the day for the month." }

    Card {
        SettingRow {
            divider: false
            title: "Calendars"
            subtitle: "The agenda shows the events of the calendars in Calendar & Weather"
            Button { text: "Open"; onClicked: SettingsState.open("calendar") }
        }
    }
}
