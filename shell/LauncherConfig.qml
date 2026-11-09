// How often each app was launched from the launcher, for ranking.
// Persisted to $XDG_CONFIG_HOME/whiterose/launcher.json.
pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io

Singleton {
    id: root
    readonly property var launches: adapter.launches

    function record(id) {
        const l = Object.assign({}, adapter.launches);
        l[id] = (l[id] ?? 0) + 1;
        adapter.launches = l;
    }

    FileView {
        path: DockConfig.configDir + "/launcher.json"
        watchChanges: true
        onFileChanged: reload()
        onAdapterUpdated: writeAdapter()
        onLoadFailed: error => { if (error === FileViewError.FileNotFound) writeAdapter() }
        JsonAdapter {
            id: adapter
            property var launches: ({})
        }
    }
}
