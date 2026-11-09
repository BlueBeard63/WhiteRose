// Design tokens from the Figma file (Components board).
pragma Singleton
import QtQuick
import Quickshell

Singleton {
    readonly property color bg: "#0F0F0F"
    readonly property color surface1: "#1C1C1F"
    readonly property color surface2: "#2A2A2E"
    readonly property color text: "#F4F4F5"
    readonly property color muted: "#8E8E96"
    readonly property color faint: "#5A5A62"
    readonly property color rose: "#E8A3AE"
    readonly property color pearl: "#F3E6E8"
    readonly property color grip: "#3A3A3E"

    // Bundled fonts (shell/fonts, OFL; QuickShell only loads files inside its config dir). Loaded from the repo so the shell
    // looks right even before the package installs them system-wide.
    readonly property string font: "Geist"
    readonly property string mono: "Geist Mono"
    readonly property list<FontLoader> fonts: [
        FontLoader { source: Qt.resolvedUrl("fonts/Geist-Regular.ttf") },
        FontLoader { source: Qt.resolvedUrl("fonts/Geist-Medium.ttf") },
        FontLoader { source: Qt.resolvedUrl("fonts/Geist-SemiBold.ttf") },
        FontLoader { source: Qt.resolvedUrl("fonts/GeistMono-Regular.ttf") },
        FontLoader { source: Qt.resolvedUrl("fonts/GeistMono-Medium.ttf") }
    ]

    // Figma sizes are in px; Qt fonts accept fractional points (1px = 0.75pt at 96 dpi).
    function pt(px) { return px * 0.75 }

    readonly property int barHeight: 26
    readonly property int fastAnim: 180
}
