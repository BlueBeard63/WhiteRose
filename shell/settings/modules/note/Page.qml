// Note: the island's note (SUPER+SHIFT+N). One text, its first line the title,
// "[ ]" lines tasks; turned off in Settings › Modules, SUPER+SHIFT+N does nothing.
import QtQuick
import qs
import qs.settings
import "../../../Note.js" as Note

Column {
    id: page
    spacing: 16
    readonly property string text: Daemon.setting("note.text", "")
    readonly property var note: Note.parse(page.text)

    PageHeader { title: "Note"; subtitle: "A note in the island, opened with SUPER+SHIFT+N. Its first line is the title; lines starting [ ] are tasks." }

    Card {
        SettingRow {
            divider: false
            title: page.note.title || "Nothing written yet"
            subtitle: page.note.tasks > 0 ? page.note.done + " of " + page.note.tasks + " tasks done" : page.note.lines.length + (page.note.lines.length === 1 ? " line" : " lines")
        }
        SettingRow {
            title: "Finished tasks"
            subtitle: "Take the ticked ones off the list"
            Button {
                text: "Clear"
                enabled: page.note.done > 0
                onClicked: Daemon.set("note.text", Note.withoutDone(page.text))
            }
        }
        SettingRow {
            title: "The whole note"
            Button {
                text: "Clear"
                kind: "ghost"
                enabled: page.text !== ""
                onClicked: Daemon.set("note.text", "")
            }
        }
    }
}
