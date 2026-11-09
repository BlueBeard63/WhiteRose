// Design tokens from the Figma file (Components board), following the
// Appearance settings live: dark, light or auto (light from sunrise to
// sunset at the weather location), the accent colour, the font and the
// text size. Every surface reads these, so a change restyles everything.
pragma Singleton
import QtQuick
import Quickshell
import "Sun.js" as Sun

Singleton {
    id: root

    // ---- settings -----------------------------------------------------------
    readonly property string mode: Daemon.setting("appearance.theme", "dark")
    readonly property bool light: mode === "light" || (mode === "auto" && daytime)
    // The accent: toggles, highlights, the active workspace.
    readonly property color rose: Daemon.setting("appearance.accent", "#E8A3AE")
    readonly property string size: Daemon.setting("appearance.size", "default")
    readonly property real textScale: size === "compact" ? 0.92 : size === "large" ? 1.1 : 1

    // Auto: day or night where the weather is set, else 07:00-19:00. Checked
    // again only when it next changes (a single timer, no polling).
    readonly property var latitude: Daemon.setting("weather.latitude", null)
    readonly property var longitude: Daemon.setting("weather.longitude", null)
    property bool daytime: true
    function checkSun() {
        const now = Date.now();
        root.daytime = Sun.isDay(now, root.latitude, root.longitude);
        sunTimer.interval = Sun.untilChange(now, root.latitude, root.longitude);
        sunTimer.restart();
    }
    onLatitudeChanged: checkSun()
    onLongitudeChanged: checkSun()
    Component.onCompleted: checkSun()
    Timer { id: sunTimer; running: root.mode === "auto"; onTriggered: root.checkSun() }
    onModeChanged: if (mode === "auto") checkSun()

    // ---- colours ----------------------------------------------------------------
    readonly property color bg: light ? "#F4F3F1" : "#0F0F0F"
    readonly property color surface1: light ? "#FFFFFF" : "#1C1C1F"
    readonly property color surface2: light ? "#EAE8E6" : "#2A2A2E"
    readonly property color text: light ? "#18171A" : "#F4F4F5"
    readonly property color muted: light ? "#6E6C74" : "#8E8E96"
    readonly property color faint: light ? "#A3A1A8" : "#5A5A62"
    // Selected chips and primary buttons: the strongest contrast to bg.
    readonly property color pearl: light ? "#1F1D22" : "#F3E6E8"
    readonly property color grip: light ? "#D9D7D4" : "#3A3A3E"
    // The Settings sidebar, a shade off the window.
    readonly property color sidebar: light ? "#ECEBE8" : "#151517"
    // On the accent colour (all accents are light pastels): dark in both themes.
    readonly property color onAccent: "#141216"
    // The accent faintly over the background (a highlighted row, a toast).
    function tint(a) { return Qt.rgba(root.rose.r, root.rose.g, root.rose.b, a) }

    // ---- type -----------------------------------------------------------------
    // Bundled fonts (shell/fonts, OFL; QuickShell only loads files inside its config dir). Loaded from the repo so the shell
    // looks right even before the package installs them system-wide.
    readonly property string font: Daemon.setting("appearance.font", "Geist")
    readonly property string mono: "Geist Mono"
    readonly property list<FontLoader> fonts: [
        FontLoader { source: Qt.resolvedUrl("fonts/Geist-Regular.ttf") },
        FontLoader { source: Qt.resolvedUrl("fonts/Geist-Medium.ttf") },
        FontLoader { source: Qt.resolvedUrl("fonts/Geist-SemiBold.ttf") },
        FontLoader { source: Qt.resolvedUrl("fonts/GeistMono-Regular.ttf") },
        FontLoader { source: Qt.resolvedUrl("fonts/GeistMono-Medium.ttf") }
    ]

    // Figma sizes are in px; Qt fonts accept fractional points (1px = 0.75pt
    // at 96 dpi), scaled by the interface size.
    function pt(px) { return px * 0.75 * root.textScale }

    readonly property int barHeight: 26
    readonly property int fastAnim: 180
}
