// Apps › Default apps: which app opens each kind of file in WhiteRose
// (whiterosed's default_apps ops, internal/mimeapps). A category sets all
// its types at once; a search sets one type (".docx", "epub"). Saved in
// ~/.config/whiterose-mimeapps.list: GNOME keeps its own. Files' "Opens
// with" saves to the same place.
import QtQuick
import Quickshell
import qs
import qs.settings

Column {
    id: page
    spacing: 16

    property var state: null
    property var found: []
    property string query: ""
    property string error: ""

    function refresh() {
        Daemon.request({ op: "default_apps" }, (v, err) => {
            if (!page) return;
            page.error = err ?? "";
            if (v) page.state = v;
        });
        if (page.query.trim() !== "") page.search();
    }
    function search() {
        Daemon.request({ op: "default_apps_search", name: page.query }, v => { if (page) page.found = v ?? [] });
    }
    function set(key, id) {
        Daemon.request({ op: "default_apps_set", key: key, name: id }, (v, err) => {
            if (!page) return;
            page.error = err ?? "";
            page.refresh();
        });
    }
    function reset(key) {
        Daemon.request({ op: "default_apps_reset", key: key }, (v, err) => {
            if (!page) return;
            page.error = err ?? "";
            page.refresh();
        });
    }
    Component.onCompleted: refresh()
    Timer { id: searchSoon; interval: 200; onTriggered: page.search() }

    // ".docx, .dotx" or the type itself.
    function typeName(t) { return (t.exts ?? []).length ? t.exts.slice(0, 4).join(", ") : t.mime }

    PageHeader {
        title: "Default apps"
        subtitle: "What opens each kind of file in WhiteRose. GNOME keeps its own."
    }

    Text {
        visible: page.error !== ""
        width: parent.width
        wrapMode: Text.Wrap
        text: page.error
        color: Theme.rose
        font { family: Theme.font; pointSize: Theme.pt(11.5) }
    }

    Card {
        Repeater {
            model: page.state?.categories ?? []
            delegate: SettingRow {
                id: catRow
                required property var modelData
                required property int index
                divider: index > 0
                title: modelData.name
                subtitle: modelData.apps.length === 0 ? "Nothing installed opens these" : modelData.changed ? "Set by you" : ""
                Button {
                    visible: catRow.modelData.changed
                    text: "Reset"; kind: "ghost"
                    label: "Reset " + catRow.modelData.name
                    onClicked: page.reset(catRow.modelData.key)
                }
                Choice {
                    enabled: catRow.modelData.apps.length > 0
                    value: catRow.modelData.current?.id ?? ""
                    options: catRow.modelData.apps.map(a => ({ label: a.name, value: a.id }))
                    onPicked: v => page.set(catRow.modelData.key, v)
                }
            }
        }
    }

    // ---- one kind of file -------------------------------------------------------------
    Section { text: "By file type" }
    Field {
        width: parent.width
        placeholder: "Find a type: .docx, epub, csv…"
        label: "Find a file type"
        text: page.query
        onTextChanged: { page.query = text; if (text.trim() === "") page.found = []; else searchSoon.restart() }
    }
    Card {
        visible: page.query.trim() !== ""
        SettingRow {
            visible: page.found.length === 0
            divider: false
            title: "No file type matches “" + page.query.trim() + "”"
        }
        Repeater {
            model: page.found
            delegate: SettingRow {
                id: typeRow
                required property var modelData
                required property int index
                divider: index > 0
                title: page.typeName(modelData)
                subtitle: modelData.mime + (modelData.changed ? " · set by you" : "")
                Button {
                    visible: typeRow.modelData.changed
                    text: "Reset"; kind: "ghost"
                    label: "Reset " + typeRow.title
                    onClicked: page.reset("type:" + typeRow.modelData.mime)
                }
                Choice {
                    enabled: typeRow.modelData.apps.length > 0
                    value: typeRow.modelData.current?.id ?? ""
                    options: typeRow.modelData.apps.length > 0
                        ? typeRow.modelData.apps.map(a => ({ label: a.name, value: a.id }))
                        : [{ label: "No app opens this", value: "" }]
                    onPicked: v => page.set("type:" + typeRow.modelData.mime, v)
                }
            }
        }
    }

    // ---- what you've set yourself ---------------------------------------------------------
    Section { text: "Set by you"; visible: (page.state?.changed ?? []).length > 0 }
    Card {
        visible: (page.state?.changed ?? []).length > 0
        Repeater {
            model: page.state?.changed ?? []
            delegate: SettingRow {
                id: mineRow
                required property var modelData
                required property int index
                divider: index > 0
                title: page.typeName(modelData)
                subtitle: modelData.mime + " · " + (modelData.current?.name ?? "app not installed")
                Button {
                    text: "Reset"; kind: "ghost"
                    label: "Reset " + mineRow.title
                    onClicked: page.reset("type:" + mineRow.modelData.mime)
                }
            }
        }
    }
}
