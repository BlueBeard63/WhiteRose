// Calendar & weather: the calendars the clock panel shows (iCal links, kept
// in the keyring, or .ics files) and where the weather is for.
import QtQuick
import Quickshell
import qs
import qs.settings

Column {
    id: page
    spacing: 16

    property var calendars: []
    property string error: ""
    function refresh() { Daemon.request({ op: "calendars" }, (v, err) => { if (!err) page.calendars = v ?? [] }) }
    Component.onCompleted: refresh()
    Connections {
        target: Daemon
        function onSettingsChanged() { page.refresh() }
    }

    PageHeader { title: "Calendar & weather"; subtitle: "What the clock panel shows." }

    Section { text: "Calendars" }
    Card {
        SettingRow {
            visible: page.calendars.length === 0
            divider: false
            title: "No calendars"
            subtitle: "Add a calendar link, or drop .ics files into ~/.local/share/whiterose/calendars."
        }
        Repeater {
            model: page.calendars
            delegate: SettingRow {
                required property var modelData
                required property int index
                divider: index > 0
                title: modelData.name
                subtitle: !modelData.has_link ? modelData.path
                    : modelData.locked ? "Link · the keyring is locked, so it isn't updated"
                    : "Link · kept in the keyring"
                Rectangle { width: 12; height: 12; radius: 6; color: modelData.color; anchors.verticalCenter: parent.verticalCenter }
                Button {
                    text: "Remove"; kind: "danger"
                    onClicked: Daemon.request({ op: "remove_calendar", index: parent.parent.index }, (v, err) => { page.error = err ?? ""; page.refresh() })
                }
            }
        }
    }
    Card {
        SettingRow {
            divider: false
            title: "Add a calendar"
            subtitle: "Google: calendar settings → “Secret address in iCal format”. Outlook and iCloud have the same."
        }
        SettingRow {
            title: "Name"
            Field { id: calName; implicitWidth: 300; placeholder: "Personal" }
        }
        SettingRow {
            title: "Link or file"
            Field { id: calSource; implicitWidth: 300; mono: true; placeholder: "https://… or /home/…/calendar.ics" }
        }
        SettingRow {
            title: "Colour"
            Row {
                spacing: 6
                Repeater {
                    model: ["#E8A3AE", "#8FB8F2", "#9BD5A8", "#F2C98F", "#B9A3E8", "#F2A68F"]
                    delegate: Rectangle {
                        required property string modelData
                        width: 20; height: 20; radius: 10
                        color: modelData
                        border { width: addRow.color === modelData ? 2 : 0; color: Theme.text }
                        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: addRow.color = modelData }
                    }
                }
            }
        }
        SettingRow {
            id: addRow
            property string color: "#E8A3AE"
            title: page.error
            Button {
                text: "Add"; kind: "primary"
                enabled: calName.text.trim() !== "" && calSource.text.trim() !== ""
                onClicked: {
                    const src = calSource.text.trim();
                    const req = { op: "add_calendar", name: calName.text.trim(), color: addRow.color };
                    if (src.includes("://")) req.url = src; else req.path = src;
                    Daemon.request(req, (v, err) => {
                        page.error = err ?? "";
                        if (!err) { calName.text = ""; calSource.text = ""; page.refresh(); }
                    });
                }
            }
        }
    }

    Section { text: "Weather" }
    Card {
        SettingRow {
            divider: false
            title: "Place"
            subtitle: System.weather ? System.weather.description + ", " + System.weather.temperature + "° in " + System.weather.place : "Not set: the weather stays hidden"
            Field {
                id: cityField
                implicitWidth: 220
                placeholder: "City"
                text: Daemon.setting("weather.city", "")
                onAccepted: Daemon.set("weather.city", text.trim())
            }
            Button { text: "Set"; onClicked: Daemon.set("weather.city", cityField.text.trim()) }
        }
        SettingRow {
            title: "Units"
            Segmented {
                value: Daemon.setting("weather.units", "celsius")
                options: [{ label: "°C", value: "celsius" }, { label: "°F", value: "fahrenheit" }]
                onPicked: v => Daemon.set("weather.units", v)
            }
        }
    }
}
