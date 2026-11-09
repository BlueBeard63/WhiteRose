// The wallpaper setting and the images on offer (from the wallpaper
// folders, re-listed when the folders change or `refresh()` is called).
pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import "Wallpapers.js" as W

Singleton {
    id: root
    readonly property string home: Quickshell.env("HOME") ?? ""
    readonly property string current: Daemon.setting("wallpaper.current", "builtin:rose")
    readonly property var folders: Daemon.setting("wallpaper.folders", ["~/Pictures/Wallpapers"])
    readonly property var files: Daemon.setting("wallpaper.files", [])
    readonly property int rotateMinutes: Daemon.setting("wallpaper.rotateMinutes", 0)
    // Images found in the folders.
    property var images: []
    readonly property var all: W.all(root.images, root.files.map(f => W.expand(f, root.home)))

    function set(id) { Daemon.set("wallpaper.current", id) }
    function next() { set(W.next(root.images, root.files.map(f => W.expand(f, root.home)), root.current)) }

    function refresh() {
        lister.command = ["find"].concat(root.folders.map(f => W.expand(f, root.home)))
            .concat(["-maxdepth", "1", "-type", "f"]);
        lister.running = true;
    }
    onFoldersChanged: refresh()
    Component.onCompleted: refresh()
    Process {
        id: lister
        stdout: StdioCollector { onStreamFinished: root.images = W.parseList(text).slice(0, 300) }
    }
}
