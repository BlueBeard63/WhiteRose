// The dock: pinned and running apps; hovering a running one shows its
// windows below the island (they take clicks there too).
import QtQuick
import qs

IslandView {
    overhang: dock.overhang
    inputs: [dock.previewCard]
    DockView { id: dock }
}
