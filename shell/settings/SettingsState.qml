// The Settings app's state, shared by the window and its pages: the
// modules found, which page is open.
pragma Singleton
import QtQuick
import Quickshell
import Quickshell.Io
import qs

Singleton {
    id: root

    // [{ id, name, description, icon, category, order, version, core, keywords, dir, builtin }]
    property var modules: []
    readonly property var disabled: Daemon.setting("settings.modules.disabled", [])
    readonly property var enabled: modules.filter(m => m.core || !disabled.includes(m.id))
    readonly property string userModules: (Quickshell.env("XDG_DATA_HOME") || Quickshell.env("HOME") + "/.local/share") + "/whiterose/modules"

    property string page: ""
    // Asked for before the modules were found (IPC, or WHITEROSE_SETTINGS_PAGE).
    property string wanted: Quickshell.env("WHITEROSE_SETTINGS_PAGE") ?? ""
    readonly property var current: modules.find(m => m.id === page) ?? null

    function open(id) {
        if (modules.some(m => m.id === id)) page = id;
        else wanted = id;
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
                    root.page = found.some(m => m.id === root.wanted) ? root.wanted : (root.enabled[0]?.id ?? "");
            }
        }
    }

    IpcHandler {
        target: "settings"
        function open(page: string): void { root.open(page) }
    }
}
