// The Settings app's state, shared by the window and its pages: the
// modules found, which page is open.
pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs

Singleton {
    id: root

    // [{ id, name, description, icon, category, order, version, core, keywords, dir, builtin,
    //    pages? [{ id, name, icon, file, keywords }] }]: a module's sub-pages.
    property var modules: []
    // A count beside a page in the sidebar ({ updates: 1 }).
    property var badges: ({})
    function setBadge(id, n) {
        const b = Object.assign({}, badges);
        if (n > 0) b[id] = n; else delete b[id];
        badges = b;
    }
    // Updates: 1 while there's an update (or a failed start) to see to.
    function updatesBadge(st) { setBadge("updates", st && (st.release || st.failure) ? 1 : 0) }
    Connections {
        target: Daemon
        function onUpdatesEvent(st) { root.updatesBadge(st) }
        function onConnectedChanged() {
            if (Daemon.connected) Daemon.request({ op: "updates" }, v => { if (v) root.updatesBadge(v) });
        }
    }
    Component.onCompleted: if (Daemon.connected) Daemon.request({ op: "updates" }, v => { if (v) root.updatesBadge(v) })
    readonly property var disabled: Daemon.setting("settings.modules.disabled", [])
    readonly property var enabled: modules.filter(m => m.core || !disabled.includes(m.id))
    readonly property string userModules: (Quickshell.env("XDG_DATA_HOME") || Quickshell.env("HOME") + "/.local/share") + "/whiterose/modules"

    property string page: ""
    // The open sub-page of that module ("": its own Page.qml).
    property string sub: ""
    // Asked for before the modules were found (IPC, or WHITEROSE_SETTINGS_PAGE).
    property string wanted: Quickshell.env("WHITEROSE_SETTINGS_PAGE") ?? ""
    readonly property var current: modules.find(m => m.id === page) ?? null
    readonly property var currentSub: (current?.pages ?? []).find(p => p.id === sub) ?? null

    // "module" or "module/sub-page".
    function open(id) {
        const [m, s] = String(id).split("/");
        if (modules.some(x => x.id === m)) {
            page = m;
            sub = (modules.find(x => x.id === m).pages ?? []).some(p => p.id === s) ? s : "";
        } else wanted = id;
    }
    // A page switched off while open: go to the first one still on.
    onEnabledChanged: if (page !== "" && !enabled.some(m => m.id === page)) page = enabled[0]?.id ?? ""

    Process {
        running: true
        command: ["sh", "-c", "for f in \"$1\"/*/module.json \"$2\"/*/module.json; do [ -f \"$f\" ] && printf '%s\\t%s\\n' \"${f%/module.json}\" \"$(tr '\\n' ' ' < \"$f\")\"; done; true",
            "sh", Quickshell.shellDir + "/settings/modules", root.userModules]
        stdout: StdioCollector {
            onStreamFinished: {
                const found = [];
                for (const line of text.split("\n")) {
                    const tab = line.indexOf("\t");
                    if (tab < 0) continue;
                    try {
                        const m = JSON.parse(line.slice(tab + 1));
                        if (!m.id || !m.name || found.some(f => f.id === m.id)) continue;   // built-in wins
                        m.dir = line.slice(0, tab);
                        m.builtin = m.dir.startsWith(Quickshell.shellDir);
                        found.push(m);
                    } catch (e) { console.warn("settings: bad module.json in", line.slice(0, tab), e); }
                }
                found.sort((a, b) => (a.order ?? 50) - (b.order ?? 50) || a.name.localeCompare(b.name));
                root.modules = found;
                if (root.page === "" || !found.some(m => m.id === root.page))
                    if (found.some(m => m.id === String(root.wanted).split("/")[0])) root.open(root.wanted);
                    else root.page = root.enabled[0]?.id ?? "";
            }
        }
    }

    IpcHandler {
        target: "settings"
        function open(page: string): void { root.open(page) }
    }
}
