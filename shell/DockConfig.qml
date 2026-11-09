// Pinned dock apps: the "dock.pinned" setting (in whiterosed's settings
// database; the default is the Figma dock's apps).
pragma Singleton
import QtQuick
import Quickshell

Singleton {
    id: root

    // Desktop entry ids, in dock order. Missing apps are skipped, not shown broken.
    readonly property var pinned: Daemon.setting("dock.pinned", [])

    function isPinned(id) { return root.pinned.includes(id) }
    function pin(id) { if (!isPinned(id)) Daemon.set("dock.pinned", [...root.pinned, id]) }
    function unpin(id) { Daemon.set("dock.pinned", root.pinned.filter(p => p !== id)) }
    function togglePin(id) { isPinned(id) ? unpin(id) : pin(id) }
}
