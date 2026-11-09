// Pinned dock apps, persisted to $XDG_CONFIG_HOME/whiterose/dock.json and
// reloaded if the file is edited by hand.
pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

Singleton {
    id: root

    // Desktop entry ids, in dock order. Missing apps are skipped, not shown broken.
    readonly property list<string> pinned: adapter.pinned

    function isPinned(id) { return adapter.pinned.includes(id) }
    function pin(id) { if (!isPinned(id)) adapter.pinned = [...adapter.pinned, id] }
    function unpin(id) { adapter.pinned = adapter.pinned.filter(p => p !== id) }
    function togglePin(id) { isPinned(id) ? unpin(id) : pin(id) }

    readonly property string configDir:
        (Quickshell.env("XDG_CONFIG_HOME") || Quickshell.env("HOME") + "/.config") + "/whiterose"

    FileView {
        path: root.configDir + "/dock.json"
        watchChanges: true
        onFileChanged: reload()
        // Save whenever the pins change.
        onAdapterUpdated: writeAdapter()
        // First run: write the defaults so the file exists to edit.
        onLoadFailed: error => { if (error === FileViewError.FileNotFound) writeAdapter() }

        JsonAdapter {
            id: adapter
            // Defaults match the Figma dock: Files, Browser, Terminal, Code, Music, Messages.
            property list<string> pinned: [
                "org.gnome.Nautilus",
                "firefox_firefox",
                "org.gnome.Ptyxis",
                "code_code",
                "spotify",
                "discord_discord"
            ]
        }
    }
}
