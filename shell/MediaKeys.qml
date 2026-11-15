// Volume, microphone, brightness and media keys (the compositor turns the
// key presses into actions; see the Media group in Keyboard & shortcuts).
// Volume and brightness move in 5% steps.
import QtQuick
import Quickshell
import Quickshell.Services.Pipewire

Scope {
    id: root
    readonly property var sink: Pipewire.defaultAudioSink
    readonly property var source: Pipewire.defaultAudioSource
    readonly property var player: NowPlaying.player
    PwObjectTracker { objects: [root.sink, root.source].filter(o => o) }

    function volume(delta) {
        if (!sink?.audio) return;
        sink.audio.muted = false;
        sink.audio.volume = Math.max(0, Math.min(1, Math.round((sink.audio.volume + delta) * 20) / 20));
    }
    function brightness(delta) {
        if (System.brightness >= 0) System.setBrightness(System.brightness + delta);
    }

    Connections {
        target: Compositor
        function onAction(name) {
            switch (name) {
            case "volume_up": root.volume(0.05); break;
            case "volume_down": root.volume(-0.05); break;
            case "volume_mute": if (root.sink?.audio) root.sink.audio.muted = !root.sink.audio.muted; break;
            case "mic_mute": if (root.source?.audio) root.source.audio.muted = !root.source.audio.muted; break;
            case "brightness_up": root.brightness(5); break;
            case "brightness_down": root.brightness(-5); break;
            case "media_play_pause": if (root.player?.canTogglePlaying) root.player.togglePlaying(); break;
            case "media_next": if (root.player?.canGoNext) root.player.next(); break;
            case "media_previous": if (root.player?.canGoPrevious) root.player.previous(); break;
            }
        }
    }
}
