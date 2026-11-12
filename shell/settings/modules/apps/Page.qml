// Apps: the apps WhiteRose comes with, installed from the right place for
// this distro (its own packages, the maker's repository, or Flathub). Pick
// some, Install, and the password prompt asks once; progress shows as it
// goes. Also opens by itself on first login (the first-run setup).
import QtQuick
import Quickshell
import qs
import qs.settings

Column {
    id: page
    spacing: 16

    property var info: null          // { distro, apps: [{ id, name, what, source, community, installed }] }
    property string loadError: ""
    property var picked: ({})        // id -> true
    property bool busy: false
    property string error: ""
    // Progress, by step label: { label, state, line, error } in order.
    property var steps: []
    readonly property var apps: info?.apps ?? []
    readonly property var missing: apps.filter(a => !a.installed)
    readonly property var chosen: apps.filter(a => page.picked[a.id])

    function refresh() {
        Daemon.request({ op: "apps" }, (v, err) => {
            page.loadError = err ?? "";
            if (!err) {
                page.info = v;
                // Start with everything not installed yet picked.
                if (Object.keys(page.picked).length === 0) {
                    const p = {};
                    for (const a of v.apps) if (!a.installed) p[a.id] = true;
                    page.picked = p;
                }
            }
        });
    }
    Component.onCompleted: refresh()

    function toggle(id) {
        const p = Object.assign({}, page.picked);
        if (p[id]) delete p[id]; else p[id] = true;
        page.picked = p;
    }
    function install() {
        page.busy = true;
        page.error = "";
        page.steps = [];
        Daemon.request({ op: "apps_install", args: { ids: page.chosen.map(a => a.id) } }, (v, err) => {
            page.busy = false;
            page.error = err ?? "";
            if (v) { page.info = v; page.picked = {}; }
            else page.refresh();
        });
    }
    Connections {
        target: Daemon
        function onAppsEvent(e) {
            const list = page.steps.slice();
            const i = list.findIndex(s => s.label === e.step);
            const s = { label: e.step, state: e.state, line: e.line || (i >= 0 ? list[i].line : ""), error: e.error ?? "" };
            if (i >= 0) list[i] = s; else list.push(s);
            page.steps = list;
        }
    }

    function sourceText(a) {
        const where = { distro: page.info?.distro === "fedora" ? "Fedora" : "Ubuntu", vendor: "The maker's repository", copr: "COPR", flathub: "Flathub" }[a.source] ?? a.source;
        return a.what + " · " + where + (a.community ? " (community build)" : "");
    }

    PageHeader {
        title: "Apps"
        subtitle: "The apps WhiteRose comes with, from the right place for this computer."
        Button {
            text: page.busy ? "Installing…" : page.chosen.length > 0 ? "Install " + page.chosen.length : "Install"
            kind: "primary"
            enabled: !page.busy && page.chosen.length > 0
            onClicked: page.install()
        }
    }

    SettingRow {
        visible: page.loadError !== ""
        title: "The app setup isn't available"
        subtitle: page.loadError
    }

    // ---- progress -------------------------------------------------------------------
    Card {
        visible: page.steps.length > 0 || page.error !== ""
        Repeater {
            model: page.steps
            delegate: Item {
                required property var modelData
                required property int index
                width: parent.width; height: 48
                Rectangle { visible: index > 0; width: parent.width; height: 1; color: Theme.surface2 }
                Rectangle {
                    id: dot
                    x: 16; anchors.verticalCenter: parent.verticalCenter
                    width: 20; height: 20; radius: 10
                    color: modelData.state === "done" ? Theme.rose : modelData.state === "failed" ? Theme.tint(0.2) : Theme.surface2
                    Icon {
                        anchors.centerIn: parent
                        visible: modelData.state !== "running"
                        name: modelData.state === "done" ? "check" : "x"
                        size: 11
                        color: modelData.state === "done" ? Theme.onAccent : Theme.rose
                    }
                    // Running: a ring that turns.
                    Rectangle {
                        visible: modelData.state === "running"
                        anchors.centerIn: parent
                        width: 12; height: 12; radius: 6
                        color: "transparent"
                        border { width: 2; color: Theme.muted }
                        Rectangle { width: 4; height: 4; radius: 2; color: Theme.text; x: 4; y: -1 }
                        RotationAnimation on rotation { running: parent.visible; loops: Animation.Infinite; from: 0; to: 360; duration: 900 }
                    }
                }
                Column {
                    anchors { left: dot.right; leftMargin: 12; right: parent.right; rightMargin: 16; verticalCenter: parent.verticalCenter }
                    spacing: 1
                    Text { text: modelData.label; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(12.5); weight: Font.Medium } }
                    Text {
                        width: parent.width
                        visible: text !== ""
                        elide: Text.ElideRight
                        text: modelData.error || (modelData.state === "running" ? modelData.line : "")
                        color: modelData.error ? Theme.rose : Theme.faint
                        font { family: Theme.mono; pointSize: Theme.pt(10.5) }
                    }
                }
            }
        }
        SettingRow {
            visible: page.error !== ""
            title: "Not everything installed"
            subtitle: page.error
        }
    }

    // ---- the apps -----------------------------------------------------------------------
    Section { visible: page.apps.length > 0; text: page.missing.length === 0 ? "All installed" : "Choose what to install" }
    Card {
        visible: page.apps.length > 0
        Repeater {
            model: page.apps
            delegate: Item {
                id: row
                required property var modelData
                required property int index
                readonly property bool on: !!page.picked[modelData.id]
                width: parent.width; height: 56
                Rectangle { visible: row.index > 0; width: parent.width; height: 1; color: Theme.surface2 }
                AppIcon {
                    id: icon
                    x: 16; anchors.verticalCenter: parent.verticalCenter
                    size: 26
                    entry: null
                    name: row.modelData.name
                    iconName: ({ ghostty: "com.mitchellh.ghostty", vscode: "code", thunderbird: "thunderbird", bitwarden: "bitwarden",
                                 libreoffice: "libreoffice-main", vlc: "vlc", calculator: "org.gnome.Calculator", zapzap: "com.rtosta.zapzap",
                                 discord: "discord", signal: "signal-desktop", zsh: "utilities-terminal", git: "git" })[row.modelData.id] ?? ""
                }
                Column {
                    // Up to its badge or switch: a long description is cut short there.
                    anchors { left: icon.right; leftMargin: 12; right: parent.right; rightMargin: 100; verticalCenter: parent.verticalCenter }
                    spacing: 1
                    Text { width: parent.width; elide: Text.ElideRight; text: row.modelData.name; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.Medium } }
                    Text { width: parent.width; elide: Text.ElideRight; text: page.sourceText(row.modelData); color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11) } }
                }
                Badge {
                    visible: row.modelData.installed
                    anchors { right: parent.right; rightMargin: 16; verticalCenter: parent.verticalCenter }
                    text: "installed"
                }
                Toggle {
                    label: row.modelData.name
                    visible: !row.modelData.installed
                    anchors { right: parent.right; rightMargin: 16; verticalCenter: parent.verticalCenter }
                    checked: row.on
                    enabled: !page.busy
                    onToggled: page.toggle(row.modelData.id)
                }
            }
        }
    }
    Text {
        visible: page.apps.length > 0
        width: parent.width
        wrapMode: Text.Wrap
        text: "Zsh comes with Oh My Zsh and your theme, and becomes your shell. VS Code comes with your extensions. Community builds are packaged by others than the app's makers."
        color: Theme.faint
        font { family: Theme.font; pointSize: Theme.pt(11.5) }
    }

    // First run: once seen, it doesn't open by itself again.
    Component.onDestruction: if (!Daemon.setting("apps.setupDone", false)) Daemon.set("apps.setupDone", true)
}
