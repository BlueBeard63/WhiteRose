// The desktop's look for WhiteRose's apps: the same tokens as the shell's
// Theme.qml, following Settings > Appearance live (through Desktop).
pragma Singleton
import QtQuick
import WhiteRose.Kit

QtObject {
    id: root
    readonly property string mode: Desktop.theme
    // Auto: checked every minute (cheap) and whenever the theme setting changes.
    property bool daytime: Desktop.daytime(new Date().getHours())
    readonly property bool light: mode === "light" || (mode === "auto" && daytime)
    readonly property Timer sunTimer: Timer {
        interval: 60000; repeat: true; running: root.mode === "auto"
        onTriggered: root.daytime = Desktop.daytime(new Date().getHours())
    }
    onModeChanged: root.daytime = Desktop.daytime(new Date().getHours())

    readonly property color bg: light ? "#F4F3F1" : "#0F0F0F"
    readonly property color surface1: light ? "#FFFFFF" : "#1C1C1F"
    readonly property color surface2: light ? "#EAE8E6" : "#2A2A2E"
    readonly property color text: light ? "#18171A" : "#F4F4F5"
    readonly property color muted: light ? "#6E6C74" : "#8E8E96"
    readonly property color faint: light ? "#A3A1A8" : "#5A5A62"
    readonly property color pearl: light ? "#1F1D22" : "#F3E6E8"
    readonly property color grip: light ? "#D9D7D4" : "#3A3A3E"
    readonly property color sidebar: light ? "#ECEBE8" : "#151517"
    readonly property color rose: Desktop.accent
    readonly property color onAccent: "#141216"
    function tint(a) { return Qt.rgba(root.rose.r, root.rose.g, root.rose.b, a) }

    readonly property string font: Desktop.font
    readonly property string mono: "Geist Mono"
    readonly property real textScale: Desktop.size === "compact" ? 0.92 : Desktop.size === "large" ? 1.1 : 1
    function pt(px) { return px * 0.75 * root.textScale }
    readonly property int fastAnim: 180

    // Geist (bundled), so apps look right wherever they run.
    readonly property list<FontLoader> fonts: [
        FontLoader { source: Qt.resolvedUrl("../fonts/Geist-Regular.ttf") },
        FontLoader { source: Qt.resolvedUrl("../fonts/Geist-Medium.ttf") },
        FontLoader { source: Qt.resolvedUrl("../fonts/Geist-SemiBold.ttf") },
        FontLoader { source: Qt.resolvedUrl("../fonts/GeistMono-Regular.ttf") },
        FontLoader { source: Qt.resolvedUrl("../fonts/GeistMono-Medium.ttf") }
    ]
}
