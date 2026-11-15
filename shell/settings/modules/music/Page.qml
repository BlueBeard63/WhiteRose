// Music: what's playing, in the island (the cover, the track, its
// controls), from "Now playing" in the bar or SUPER+SHIFT+M. Turned off in
// Settings › Modules, "Now playing" plays and pauses, as before.
import QtQuick
import qs
import qs.settings

Column {
    id: page
    spacing: 16

    PageHeader { title: "Music"; subtitle: "What's playing, in the island: click Now playing in the bar, or SUPER+SHIFT+M." }

    Card {
        SettingRow {
            divider: false
            title: "Now playing in the bar"
            subtitle: "The track and its moving bars, on the left of the bar"
            Toggle { checked: Daemon.setting("bar.media", true); onToggled: on => Daemon.set("bar.media", on) }
        }
    }
}
