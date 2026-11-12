// Island view: what's playing (Figma: "Island v2 — Music (art)"; the Music
// module, "Now playing" in the bar or SUPER+SHIFT+M). The cover fills the
// island's left side and fades into it; on the right the track, where it's
// playing, the position (click to seek) and the controls the player has.
// Space plays or pauses, Left / Right skip 5 s, Esc closes.
import QtQuick
import Quickshell.Services.Mpris
import Quickshell.Services.Pipewire
import Quickshell.Widgets
import qs

Item {
    id: root
    implicitWidth: 600
    implicitHeight: 178

    readonly property var player: Mpris.players.values.length > 0 ? Mpris.players.values[0] : null
    readonly property var sink: Pipewire.defaultAudioSink
    PwObjectTracker { objects: root.sink ? [root.sink] : [] }
    readonly property real length: root.player?.lengthSupported ? root.player.length : 0
    readonly property real position: root.player?.positionSupported ? root.player.position : 0

    function clock(s) { s = Math.max(0, Math.floor(s)); return Math.floor(s / 60) + ":" + String(s % 60).padStart(2, "0") }
    function seekTo(s) { if (root.player?.canSeek) root.player.position = Math.max(0, Math.min(root.length, s)) }

    // The position moves only when asked: once a second while it plays.
    Timer { interval: 1000; repeat: true; running: root.visible && !!root.player?.isPlaying; onTriggered: root.player.positionChanged() }
    Component.onCompleted: keys.forceActiveFocus()
    Item {
        id: keys
        focus: true
        Keys.onEscapePressed: ShellState.close()
        Keys.onSpacePressed: if (root.player?.canTogglePlaying) root.player.togglePlaying()
        Keys.onLeftPressed: root.seekTo(root.position - 5)
        Keys.onRightPressed: root.seekTo(root.position + 5)
    }

    // ---- the cover, filling the left side ----------------------------------------------
    // (Rounded as the island's lower corner, which is 40 at this height:
    // the island doesn't cut what a view draws.)
    ClippingRectangle {
        id: art
        width: 260; height: root.height
        color: "transparent"
        bottomLeftRadius: 40
        Rectangle {
            anchors.fill: parent
            visible: cover.status !== Image.Ready
            gradient: Gradient {
                orientation: Gradient.Vertical
                GradientStop { position: 0; color: "#E3A0B0" }
                GradientStop { position: 0.55; color: "#8E6A9E" }
                GradientStop { position: 1; color: "#2B2453" }
            }
        }
        Image {
            id: cover
            anchors.fill: parent
            source: root.player?.trackArtUrl ?? ""
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            sourceSize: Qt.size(520, 520)
        }
        // It bleeds into the island.
        Rectangle {
            x: parent.width - width; width: 150; height: parent.height
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0; color: "transparent" }
                GradientStop { position: 1; color: Theme.bg }
            }
        }
    }

    // ---- the track ---------------------------------------------------------------------
    Column {
        id: info
        x: 236; y: 20
        width: root.width - x - 24
        spacing: 4
        Text {
            width: parent.width
            elide: Text.ElideRight
            text: root.player?.trackTitle || "Nothing playing"
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(15); weight: Font.DemiBold }
            Accessible.role: Accessible.StaticText
            Accessible.name: text
        }
        Text {
            width: parent.width
            elide: Text.ElideRight
            visible: text !== ""
            text: [root.player?.trackArtist, root.player?.trackAlbum].filter(s => s).join(" · ")
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(10.5) }
        }
        Row {
            visible: !!root.sink
            spacing: 6
            topPadding: 2
            Icon { anchors.verticalCenter: parent.verticalCenter; name: "headphones"; size: 12; color: Theme.faint }
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: root.sink?.description ?? ""
                color: Theme.faint
                elide: Text.ElideRight
                width: Math.min(implicitWidth, info.width - 20)
                font { family: Theme.mono; pointSize: Theme.pt(9) }
            }
        }
    }

    // ---- where it's got to ------------------------------------------------------------
    Item {
        id: progress
        x: info.x; y: 104
        width: info.width; height: 14
        visible: root.length > 0
        Text { id: posText; anchors.verticalCenter: parent.verticalCenter; text: root.clock(root.position); color: Theme.muted; font { family: Theme.mono; pointSize: Theme.pt(9) } }
        Text { id: lenText; anchors { right: parent.right; verticalCenter: parent.verticalCenter } text: root.clock(root.length); color: Theme.muted; font { family: Theme.mono; pointSize: Theme.pt(9) } }
        Item {
            id: rail
            anchors { left: posText.right; leftMargin: 10; right: lenText.left; rightMargin: 10; verticalCenter: parent.verticalCenter }
            height: 14
            readonly property real done: root.length > 0 ? Math.min(1, root.position / root.length) : 0
            Accessible.role: Accessible.Slider
            Accessible.name: "Position"
            Rectangle { anchors.verticalCenter: parent.verticalCenter; width: parent.width; height: 3; radius: 2; color: Theme.surface2 }
            Rectangle { anchors.verticalCenter: parent.verticalCenter; width: parent.width * rail.done; height: 3; radius: 2; color: Theme.pearl }
            Rectangle { anchors.verticalCenter: parent.verticalCenter; x: parent.width * rail.done - 5; width: 10; height: 10; radius: 5; color: Theme.pearl; visible: root.player?.canSeek ?? false }
            MouseArea {
                anchors.fill: parent
                enabled: root.player?.canSeek ?? false
                cursorShape: Qt.PointingHandCursor
                onClicked: m => root.seekTo(m.x / width * root.length)
            }
        }
    }

    // ---- the controls ------------------------------------------------------------------
    Row {
        x: info.x + (info.width - width) / 2; y: 128
        spacing: 18
        Control {
            visible: root.player?.shuffleSupported ?? false
            name: "shuffle"; label: "Shuffle"
            on: root.player?.shuffle ?? false
            onPressed: root.player.shuffle = !root.player.shuffle
        }
        Control { name: "skipBack"; label: "Previous"; enabled: root.player?.canGoPrevious ?? false; onPressed: root.player.previous() }
        Control {
            name: root.player?.isPlaying ? "pause" : "play"; label: root.player?.isPlaying ? "Pause" : "Play"
            big: true
            enabled: root.player?.canTogglePlaying ?? false
            onPressed: root.player.togglePlaying()
        }
        Control { name: "skipForward"; label: "Next"; enabled: root.player?.canGoNext ?? false; onPressed: root.player.next() }
        Control {
            visible: root.player?.loopSupported ?? false
            readonly property int loop: root.player?.loopState ?? MprisLoopState.None
            name: loop === MprisLoopState.Track ? "repeat1" : "repeat"; label: "Repeat"
            on: loop !== MprisLoopState.None
            // None → the playlist → this track → none.
            onPressed: root.player.loopState = loop === MprisLoopState.None ? MprisLoopState.Playlist : loop === MprisLoopState.Playlist ? MprisLoopState.Track : MprisLoopState.None
        }
    }

    component Control: Item {
        id: c
        property string name
        property string label
        property bool big: false
        property bool on: false
        signal pressed()
        width: big ? 40 : 22; height: 40
        opacity: enabled ? 1 : 0.35
        Accessible.role: Accessible.Button
        Accessible.name: c.label
        Accessible.checkable: c.name === "shuffle" || c.label === "Repeat"
        Accessible.checked: c.on
        Accessible.onPressAction: if (c.enabled) c.pressed()
        Rectangle { anchors.centerIn: parent; visible: c.big; width: 40; height: 40; radius: 20; color: Theme.pearl }
        Icon {
            anchors.centerIn: parent
            name: c.name
            size: c.big ? 16 : 16
            color: c.big ? Theme.bg : c.on ? Theme.rose : (c.name === "shuffle" || c.label === "Repeat") ? Theme.muted : Theme.text
        }
        MouseArea { anchors.fill: parent; enabled: c.enabled; cursorShape: Qt.PointingHandCursor; onClicked: c.pressed() }
    }
}
