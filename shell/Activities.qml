// Activities (the compositor's activities.rs; Settings › Activities): the
// list and names ("activities.list"), the current one (the compositor's),
// and each one's own value of a per-activity setting ("activities.
// overrides"): dock pins, wallpaper, per-workspace layouts and zones. An
// activity uses the default activity's value (the plain key) until it has
// its own; with activities off, everything is the plain keys.
pragma Singleton
import QtQuick
import Quickshell

Singleton {
    id: root
    readonly property bool enabled: Daemon.setting("activities.enabled", false)
    readonly property var list: Daemon.setting("activities.list", [{ id: "default", name: "Main" }])
    readonly property var overrides: Daemon.setting("activities.overrides", ({}))
    // The compositor's word, else (Settings, before it connects) the saved one.
    readonly property string current: Compositor.connected ? Compositor.activity : Daemon.setting("activities.current", "default")
    // The keys an activity has its own value of (keys.go perActivity).
    readonly property var perActivity: ["dock.pinned", "wallpaper.current", "windows.layout.workspaces", "windows.snap.zones.workspaces"]

    function nameOf(id) { return root.list.find(a => a.id === id)?.name ?? "" }

    // Per-activity `key` for activity `id`: its own value, else the plain key's.
    function settingFor(id, key, fallback) {
        if (root.enabled && id !== "default") {
            const own = root.overrides[id];
            if (own && Object.prototype.hasOwnProperty.call(own, key)) return own[key];
        }
        return Daemon.setting(key, fallback);
    }
    // ... for the current activity.
    function setting(key, fallback) { return settingFor(root.current, key, fallback) }

    // Of the recently used `files` (Search.parseRecent), the current
    // activity's: the daemon tags each with the activity it was used in
    // ("activities.recentFiles"); one with no tag is the default's.
    function recentFiles(files) {
        if (!root.enabled) return files;
        const tags = Daemon.setting("activities.recentFiles", ({})) ?? {};
        return files.filter(f => (tags[f.path] ?? "default") === root.current);
    }

    // Saves `key`: a per-activity one as the current activity's own, any
    // other as it is.
    function save(key, value) {
        if (root.perActivity.includes(key)) root.set(key, value);
        else Daemon.set(key, value);
    }

    // Saves per-activity `key` as the current activity's own.
    function set(key, value) {
        if (!root.enabled || root.current === "default") {
            Daemon.set(key, value);
            return;
        }
        const all = JSON.parse(JSON.stringify(root.overrides ?? {}));
        all[root.current] = Object.assign({}, all[root.current] ?? {});
        all[root.current][key] = value;
        Daemon.set("activities.overrides", all);
    }
}
