// Video and audio (loaded by Preview.qml only when needed: it needs Qt's
// multimedia module). Quick look plays straight away; the panel waits for
// the play button. Audio shows its title and artist.
import QtQuick
import QtMultimedia
import WhiteRose.Kit
import "Format.js" as Fmt

Item {
    id: root
    property string path
    property bool big: false
    readonly property string facts: player.duration > 0 ? root.time(player.duration) : ""

    function time(ms) {
        const s = Math.floor(ms / 1000), h = Math.floor(s / 3600), m = Math.floor(s / 60) % 60;
        const ss = String(s % 60).padStart(2, "0");
        return h > 0 ? h + ":" + String(m).padStart(2, "0") + ":" + ss : m + ":" + ss;
    }

    MediaPlayer {
        id: player
        source: root.path ? Fmt.fileUrl(root.path) : ""
        audioOutput: AudioOutput {}
        videoOutput: screen
        onMediaStatusChanged: if (root.big && player.mediaStatus === MediaPlayer.LoadedMedia && player.playbackState === MediaPlayer.StoppedState) player.play()
    }
    VideoOutput {
        id: screen
        anchors { fill: parent; bottomMargin: bar.height + 8 }
        visible: player.hasVideo
        fillMode: VideoOutput.PreserveAspectFit
        MouseArea { anchors.fill: parent; onClicked: player.playbackState === MediaPlayer.PlayingState ? player.pause() : player.play() }
    }

    // Audio: what it is.
    Column {
        visible: !player.hasVideo
        anchors { centerIn: parent; verticalCenterOffset: -bar.height / 2 }
        width: parent.width - 24
        spacing: 6
        Icon { anchors.horizontalCenter: parent.horizontalCenter; name: "headphones"; size: root.big ? 64 : 40; color: Theme.muted }
        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: player.metaData.stringValue(MediaMetaData.Title)
            elide: Text.ElideRight
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(root.big ? 15 : 12.5); weight: Font.Medium }
        }
        Text {
            width: parent.width
            horizontalAlignment: Text.AlignHCenter
            text: [player.metaData.stringValue(MediaMetaData.ContributingArtist) || player.metaData.stringValue(MediaMetaData.AlbumArtist),
                   player.metaData.stringValue(MediaMetaData.AlbumTitle)].filter(x => x).join(" · ")
            elide: Text.ElideRight
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(root.big ? 12.5 : 11) }
        }
    }

    // Play / pause, where it's up to, and the bar to move along.
    Row {
        id: bar
        anchors { left: parent.left; right: parent.right; bottom: parent.bottom; margins: 4 }
        height: 30
        spacing: 8
        IconButton {
            width: 30; height: 30; iconSize: 13
            icon: player.playbackState === MediaPlayer.PlayingState ? "pause" : "play"
            onClicked: player.playbackState === MediaPlayer.PlayingState ? player.pause() : player.play()
        }
        Rectangle {
            id: track
            anchors.verticalCenter: parent.verticalCenter
            width: parent.width - 38 - 8 - clock.width - 8
            height: 4; radius: 2
            color: Theme.surface2
            Rectangle {
                width: player.duration > 0 ? parent.width * player.position / player.duration : 0
                height: 4; radius: 2
                color: Theme.rose
            }
            MouseArea {
                anchors { fill: parent; margins: -8 }
                enabled: player.seekable
                onPressed: m => player.position = player.duration * Math.max(0, Math.min(1, (m.x - 8) / track.width))
                onPositionChanged: m => { if (pressed) player.position = player.duration * Math.max(0, Math.min(1, (m.x - 8) / track.width)) }
            }
        }
        Text {
            id: clock
            anchors.verticalCenter: parent.verticalCenter
            text: root.time(player.position) + (player.duration > 0 ? " / " + root.time(player.duration) : "")
            color: Theme.muted
            font { family: Theme.mono; pointSize: Theme.pt(10.5) }
        }
    }
}
