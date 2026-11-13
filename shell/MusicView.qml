// Island view: what's playing (Figma: "Island v2 — Music (art)"; the Music
// module, "Now playing" in the bar or SUPER+SHIFT+M). The cover fills the
// island's left side and fades into it; on the right the track, where it's
// playing, the position (click to seek) and the controls the player has.
// Space plays or pauses, Left / Right skip 5 s, Esc closes.
// It moves with the music: a new song slides its title in (the way Next or
// Previous went) and fades its cover in over the last one; pausing dims the
// cover and turns the play button over; stopping fades back to the rose
// cover and "Nothing playing"; the position glides between its updates.
import QtQuick
import QtQuick.Shapes
import Quickshell.Services.Mpris
import Quickshell.Services.Pipewire
import qs

Item {
    id: root
    implicitWidth: 600
    implicitHeight: 178

    readonly property var player: NowPlaying.player
    readonly property var sink: Pipewire.defaultAudioSink
    PwObjectTracker { objects: root.sink ? [root.sink] : [] }
    readonly property real length: root.player?.lengthSupported ? root.player.length : 0
    readonly property real position: root.player?.positionSupported ? root.player.position : 0

    // ---- what's shown, which follows the player through the transitions ----
    readonly property string title: root.player?.trackTitle || "Nothing playing"
    readonly property string subtitle: [root.player?.trackArtist, root.player?.trackAlbum].filter(s => s).join(" · ")
    readonly property string artUrl: root.player?.trackArtUrl ?? ""
    readonly property bool playing: !!root.player?.isPlaying
    property string shownTitle: title
    property string shownSubtitle: subtitle
    // Which way the next title slides in: Next (1), Previous (-1); a song
    // that ends on its own goes on like Next; stopping just fades (0).
    property int pressedDir: 0
    property int swapDir: 0
    function trackChanged() {
        root.swapDir = root.pressedDir !== 0 ? root.pressedDir : root.player?.trackTitle ? 1 : 0;
        root.pressedDir = 0;
        swap.restart();
    }
    onTitleChanged: trackChanged()
    onSubtitleChanged: trackChanged()
    SequentialAnimation {
        id: swap
        ParallelAnimation {
            NumberAnimation { target: trackText; property: "opacity"; to: 0; duration: 120; easing.type: Easing.InQuad }
            NumberAnimation { target: trackText; property: "x"; to: -16 * root.swapDir; duration: 120; easing.type: Easing.InQuad }
        }
        ScriptAction {
            script: {
                root.shownTitle = root.title;
                root.shownSubtitle = root.subtitle;
                trackText.x = 16 * root.swapDir;
            }
        }
        ParallelAnimation {
            NumberAnimation { target: trackText; property: "opacity"; to: 1; duration: 220; easing.type: Easing.OutCubic }
            NumberAnimation { target: trackText; property: "x"; to: 0; duration: 260; easing.type: Easing.OutCubic }
        }
    }

    // The cover: the new one fades in over the last and settles from a
    // slight zoom; with none (stopped, or no art), the last fades away to
    // the rose gradient.
    property string lastArt: artUrl
    onArtUrlChanged: {
        coverBack.source = root.lastArt;
        coverBack.opacity = 1;
        root.lastArt = root.artUrl;
        cover.opacity = 0;
        cover.scale = 1.05;
        if (root.artUrl === "") backOut.restart();
    }
    NumberAnimation { id: backOut; target: coverBack; property: "opacity"; to: 0; duration: 360; easing.type: Easing.OutCubic }
    ParallelAnimation {
        id: coverIn
        NumberAnimation { target: cover; property: "opacity"; to: 1; duration: 360; easing.type: Easing.OutCubic }
        NumberAnimation { target: cover; property: "scale"; to: 1; duration: 700; easing.type: Easing.OutCubic }
    }

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

    // ---- the cover, filling the left side and the left ear ---------------------------
    // One outline: from the ear's tip on the bar, down the island's left
    // side, round its lower corner (40 at this height), across, and up;
    // filled with the cover (the island draws this view into its ear:
    // bleed in island/music.qml). Ear and corner as Island.qml draws them.
    readonly property real earW: 26
    readonly property real earH: 30
    readonly property real artW: 260

    Item {
        id: artSource
        width: root.earW + root.artW; height: root.height
        visible: false
        clip: true
        // Under the covers: shown with none.
        Rectangle {
            anchors.fill: parent
            gradient: Gradient {
                orientation: Gradient.Vertical
                GradientStop { position: 0; color: "#E3A0B0" }
                GradientStop { position: 0.55; color: "#8E6A9E" }
                GradientStop { position: 1; color: "#2B2453" }
            }
        }
        // The last song's cover, while the new one comes in.
        Image {
            id: coverBack
            anchors.fill: parent
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            sourceSize: Qt.size(572, 572)
        }
        Image {
            id: cover
            anchors.fill: parent
            source: root.artUrl
            fillMode: Image.PreserveAspectCrop
            asynchronous: true
            sourceSize: Qt.size(572, 572)
            onStatusChanged: {
                if (status === Image.Ready) coverIn.restart();
                // (A cover that won't load: back to the gradient, not the last song's.)
                else if (status === Image.Error) backOut.restart();
            }
            Component.onCompleted: if (status === Image.Ready) { opacity = 1; scale = 1 }
        }
        // Paused: a little dimmer.
        Rectangle {
            anchors.fill: parent
            color: "black"
            opacity: root.playing ? 0 : 0.28
            Behavior on opacity { NumberAnimation { duration: 320; easing.type: Easing.OutCubic } }
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
    ShaderEffectSource { id: artTexture; sourceItem: artSource; hideSource: true; visible: false }

    Shape {
        id: art
        x: -root.earW
        width: artSource.width; height: root.height
        preferredRendererType: Shape.CurveRenderer
        ShapePath {
            strokeWidth: -1
            fillColor: "transparent"
            fillItem: artTexture
            PathSvg {
                path: {
                    const k = 0.5523, ew = root.earW, eh = root.earH, r = 40, h = root.height, W = art.width;
                    return `M 0 0 C ${ew * k} 0 ${ew} ${eh * (1 - k)} ${ew} ${eh} L ${ew} ${h - r} `
                        + `C ${ew} ${h - r + r * k} ${ew + r - r * k} ${h} ${ew + r} ${h} L ${W} ${h} L ${W} 0 Z`;
                }
            }
        }
    }

    // ---- the track ---------------------------------------------------------------------
    Column {
        id: info
        x: 236; y: 20
        width: root.width - x - 24
        spacing: 4
        // Title and artist slide together when the song changes.
        Item {
            width: parent.width
            height: trackText.height
            Column {
                id: trackText
                width: parent.width
                spacing: 4
                Text {
                    width: parent.width
                    elide: Text.ElideRight
                    text: root.shownTitle
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(15); weight: Font.DemiBold }
                    Accessible.role: Accessible.StaticText
                    Accessible.name: text
                }
                Text {
                    width: parent.width
                    elide: Text.ElideRight
                    visible: text !== ""
                    text: root.shownSubtitle
                    color: Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(10.5) }
                }
            }
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
        opacity: root.length > 0 ? 1 : 0
        visible: opacity > 0
        Behavior on opacity { NumberAnimation { duration: 260 } }
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
            // (Gliding between the once-a-second updates, and on a seek.)
            Rectangle {
                anchors.verticalCenter: parent.verticalCenter; width: parent.width * rail.done; height: 3; radius: 2; color: Theme.pearl
                Behavior on width { NumberAnimation { duration: 300; easing.type: Easing.OutCubic } }
            }
            Rectangle {
                anchors.verticalCenter: parent.verticalCenter; x: parent.width * rail.done - 5; width: 10; height: 10; radius: 5; color: Theme.pearl; visible: root.player?.canSeek ?? false
                Behavior on x { NumberAnimation { duration: 300; easing.type: Easing.OutCubic } }
            }
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
        Control { name: "skipBack"; label: "Previous"; enabled: root.player?.canGoPrevious ?? false; onPressed: { root.pressedDir = -1; root.player.previous() } }
        Control {
            name: root.playing ? "pause" : "play"; label: root.playing ? "Pause" : "Play"
            big: true
            enabled: root.player?.canTogglePlaying ?? false
            onPressed: root.player.togglePlaying()
        }
        Control { name: "skipForward"; label: "Next"; enabled: root.player?.canGoNext ?? false; onPressed: { root.pressedDir = 1; root.player.next() } }
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
        Behavior on opacity { NumberAnimation { duration: 200 } }
        // Pressed in a little.
        scale: hit.pressed ? 0.86 : 1
        Behavior on scale { NumberAnimation { duration: 140; easing.type: Easing.OutCubic } }
        // A new icon (play ↔ pause, repeat ↔ repeat one) turns over.
        property string shownName: name
        onNameChanged: turn.restart()
        SequentialAnimation {
            id: turn
            ParallelAnimation {
                NumberAnimation { target: icon; property: "scale"; to: 0.3; duration: 90; easing.type: Easing.InQuad }
                NumberAnimation { target: icon; property: "opacity"; to: 0; duration: 90 }
            }
            ScriptAction { script: c.shownName = c.name }
            ParallelAnimation {
                NumberAnimation { target: icon; property: "scale"; to: 1; duration: 220; easing.type: Easing.OutBack }
                NumberAnimation { target: icon; property: "opacity"; to: 1; duration: 140 }
            }
        }
        Accessible.role: Accessible.Button
        Accessible.name: c.label
        Accessible.checkable: c.name === "shuffle" || c.label === "Repeat"
        Accessible.checked: c.on
        Accessible.onPressAction: if (c.enabled) c.pressed()
        Rectangle { anchors.centerIn: parent; visible: c.big; width: 40; height: 40; radius: 20; color: Theme.pearl }
        Icon {
            id: icon
            anchors.centerIn: parent
            name: c.shownName
            size: c.big ? 16 : 16
            color: c.big ? Theme.bg : c.on ? Theme.rose : (c.name === "shuffle" || c.label === "Repeat") ? Theme.muted : Theme.text
        }
        MouseArea { id: hit; anchors.fill: parent; enabled: c.enabled; cursorShape: Qt.PointingHandCursor; onClicked: c.pressed() }
    }
}
