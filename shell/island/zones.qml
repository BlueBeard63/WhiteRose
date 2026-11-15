// Snap zones edited on screen: the tools (ZoneTools.qml), in the island of
// the monitor being edited. The overlay below takes the keyboard (Escape
// ends it), so this doesn't; clicking the zones mustn't close it.
import QtQuick
import qs

IslandView {
    keyboard: false
    ZoneTools {}
}
