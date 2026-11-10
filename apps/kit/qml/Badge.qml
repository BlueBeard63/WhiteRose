// A small fact beside a name (Figma: "· WPA3", "· saved", "· core"):
// faint mono text, no box. `dot` puts a "·" before it (off when it
// stands alone, e.g. "Taken 2 min ago").
import QtQuick


Item {
    id: badge
    property string text: ""
    property bool dot: true
    property color ink: Theme.muted
    property color tint: "transparent" // (kept for old callers; not drawn)
    implicitWidth: label.implicitWidth
    implicitHeight: 16
    width: implicitWidth; height: implicitHeight
    anchors.verticalCenter: parent?.verticalCenter
    Text {
        id: label
        anchors.verticalCenter: parent.verticalCenter
        text: (badge.dot ? "· " : "") + badge.text
        color: badge.ink
        font { family: Theme.mono; pointSize: Theme.pt(10) }
    }
}
