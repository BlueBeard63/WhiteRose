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
    readonly property string current: Compositor.activity

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
