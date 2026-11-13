// The music player the shell shows and controls: the one playing (a browser
// playing YouTube is one too: browsers publish what they play), else the
// last one that played, else one with a track. "Now playing", the Music
// island and the media keys all use it, so they agree.
pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Services.Mpris

Singleton {
    id: root
    readonly property var players: Mpris.players.values
    readonly property var playing: players.find(p => p.isPlaying) ?? null
    property var last: null
    onPlayingChanged: if (playing) last = playing

    readonly property var player: playing
        ?? (players.includes(last) ? last : null)
        ?? players.find(p => (p.trackTitle ?? "") !== "")
        ?? players[0] ?? null
}
