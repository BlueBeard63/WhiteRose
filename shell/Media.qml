// Now playing (Figma: Trigger/Media + Media/EQ). Hidden when nothing is
// playing. The EQ animates only while playback is running, so a paused or
// idle bar costs nothing.
import QtQuick
import Quickshell.Services.Mpris

Trigger {
    id: root
    readonly property var player: Mpris.players.values.length > 0 ? Mpris.players.values[0] : null
    readonly property bool playing: player !== null && player.isPlaying

    visible: player !== null && (player.trackTitle ?? "") !== ""
    onClicked: if (player && player.canTogglePlaying) player.togglePlaying()

    Row {
        id: eq
        anchors.verticalCenter: parent.verticalCenter
        height: 10
        spacing: 1.5
        property int frame: 0
        // Same five frames as the Figma Media/EQ variants.
        readonly property var frames: [[6, 10, 4, 8], [10, 5, 9, 4], [4, 9, 6, 10], [8, 4, 10, 6], [5, 8, 7, 3]]

        Repeater {
            model: 4
            delegate: Rectangle {
                required property int index
                anchors.bottom: parent.bottom
                width: 2
                radius: 1
                height: root.playing ? eq.frames[eq.frame][index] : 3
                color: root.playing ? Theme.rose : Theme.muted
                Behavior on height { NumberAnimation { duration: 160; easing.type: Easing.InOutQuad } }
            }
        }

        Timer {
            interval: 180
            repeat: true
            running: root.playing && root.visible
            onTriggered: eq.frame = (eq.frame + 1) % eq.frames.length
        }
    }

    Text {
        anchors.verticalCenter: parent.verticalCenter
        text: root.player?.trackTitle ?? ""
        color: Theme.text
        font { family: Theme.font; pointSize: Theme.pt(11.5); weight: Font.Medium }
        elide: Text.ElideRight
        width: Math.min(implicitWidth, 180)
    }
    Text {
        anchors.verticalCenter: parent.verticalCenter
        text: root.player?.trackArtist ?? ""
        visible: text !== ""
        color: Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(11.5) }
        elide: Text.ElideRight
        width: Math.min(implicitWidth, 120)
    }
}
