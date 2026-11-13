// Updates › Releases: every WhiteRose release, grouped by series (0.3, 0.2)
// in cards that open like a dropdown, alpha and beta builds tucked under
// their series' release, a search, and the pre-release switch. Any release
// can be installed beside the one you're on and switched to at the next
// login (whiterosed's update_releases / update_install_version).
import QtQuick
import qs
import qs.settings
import "Updates.js" as Up

Column {
    id: page
    spacing: 16

    property var series: []
    property string error: ""
    property bool waiting: false
    property string query: ""
    // Opened series by name (the running one starts open).
    property var open: ({})
    property real now: Date.now()

    readonly property var shown: Up.filterSeries(page.series, page.query)
    readonly property bool pre: Daemon.setting("update.prerelease", false)

    function refresh() {
        Daemon.request({ op: "update_releases" }, (v, err) => {
            if (!page) return;
            page.error = err ?? "";
            if (v) {
                page.series = v;
                if (Object.keys(page.open).length === 0) {
                    const o = {};
                    const run = v.find(s => s.releases.some(r => r.state === "running")) ?? v[0];
                    if (run) o[run.name] = true;
                    page.open = o;
                }
            }
        });
    }
    // What's going on (a line under the switch), and what came of it.
    property string doing: ""
    property string note: ""
    function act(op, extra, doing, done) {
        page.waiting = true;
        page.error = "";
        page.note = "";
        page.doing = doing ?? "";
        Daemon.request(Object.assign({ op: op }, extra ?? {}), (v, err) => {
            if (!page) return;
            page.waiting = false;
            page.doing = "";
            page.error = err ?? "";
            if (!err && done) page.note = done;
            page.refresh();
        });
    }
    function install(r) {
        const v = r.version;
        page.act("update_install_version", { name: v },
            r.state === "installed" ? "Switching to " + v + "…" : "Getting " + v + " and installing it beside this version. It asks for your password…",
            "WhiteRose " + v + " starts the next time you log in.");
    }
    // Install or switch to a release; one older than the newest asks first.
    function choose(r) {
        if (r.version !== Up.newestRelease(page.series)) {
            confirmOlder.release = r;
            confirmOlder.open();
        } else {
            page.install(r);
        }
    }
    function toggle(name) {
        const o = Object.assign({}, page.open);
        o[name] = !o[name];
        page.open = o;
    }
    Component.onCompleted: refresh()
    Connections {
        target: Daemon
        function onUpdatesEvent(v) { page.refresh() }
    }

    PageHeader {
        title: "Releases"
        subtitle: "Every WhiteRose release by version. Install any of them beside the one you're on."
        Button {
            text: page.waiting && page.doing === "" ? "Checking…" : "Check now"
            icon: "refresh"
            enabled: !page.waiting
            onClicked: page.act("update_check")
        }
    }

    Field {
        width: parent.width
        placeholder: "Search releases by version or change"
        text: page.query
        onTextChanged: page.query = text
    }

    Card {
        SettingRow {
            divider: false
            title: "Pre-releases"
            subtitle: "Show alpha and beta versions, and offer them as updates. They're newer and less tested."
            Toggle {
                id: preToggle
                checked: page.pre
                onToggled: on => {
                    Daemon.set("update.prerelease", on);
                    preToggle.checked = Qt.binding(() => page.pre);
                    page.act("update_check");
                }
            }
        }
    }

    Text {
        visible: page.doing !== "" || page.note !== ""
        width: parent.width
        wrapMode: Text.Wrap
        text: page.doing || page.note
        color: Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(11.5) }
    }
    Text {
        visible: page.error !== ""
        width: parent.width
        wrapMode: Text.Wrap
        text: page.error
        color: Theme.rose
        font { family: Theme.font; pointSize: Theme.pt(11.5) }
    }

    // Nothing to show: not checked yet, or nothing matches.
    Card {
        visible: page.shown.length === 0
        SettingRow {
            divider: false
            title: page.query !== "" ? "No releases match “" + page.query.trim() + "”" : "No releases yet"
            subtitle: page.query !== "" ? "Search by version (0.3, alpha.2) or a word from the changes." : "Check now to get the list from GitHub."
        }
    }

    Repeater {
        model: page.shown
        delegate: Card {
            id: card
            required property var modelData
            readonly property bool opened: page.query !== "" || !!page.open[modelData.name]
            // A series' alpha and beta builds sit under its release, once
            // there is one; until then they're the series' only rows.
            readonly property bool hasStable: modelData.releases.some(r => !r.pre)

            // The series header: opens and closes it.
            Item {
                width: parent.width
                height: 52
                Accessible.role: Accessible.Button
                Accessible.name: "WhiteRose " + card.modelData.name
                Accessible.onPressAction: page.toggle(card.modelData.name)
                Rectangle { anchors.fill: parent; color: headMouse.containsMouse ? Theme.surface2 : "transparent" }
                Icon {
                    id: chev
                    x: 16; anchors.verticalCenter: parent.verticalCenter
                    name: card.opened ? "chevDown" : "chevR"; size: 14; color: Theme.muted
                }
                Text {
                    id: seriesName
                    anchors { left: chev.right; leftMargin: 10; verticalCenter: parent.verticalCenter }
                    text: "WhiteRose " + card.modelData.name
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(13.5); weight: Font.DemiBold }
                }
                Badge {
                    anchors { left: seriesName.right; leftMargin: 8; right: parent.right; rightMargin: 16 }
                    text: Up.seriesNote(card.modelData)
                }
                MouseArea {
                    id: headMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: page.toggle(card.modelData.name)
                }
            }

            Repeater {
                model: card.opened ? card.modelData.releases : []
                delegate: Column {
                    id: rel
                    required property var modelData
                    required property int index
                    property bool expanded: false
                    readonly property bool nested: card.hasStable && modelData.pre
                    readonly property var action: Up.releaseAction(modelData)
                    width: card.width

                    Item {
                        width: parent.width
                        height: 52
                        Rectangle { width: parent.width; height: 1; color: Theme.surface2 }
                        Rectangle { anchors.fill: parent; anchors.topMargin: 1; color: rowMouse.containsMouse ? Theme.surface2 : "transparent" }
                        // Under its series' release: a rail on the left.
                        Rectangle {
                            visible: rel.nested
                            x: 22; width: 2; radius: 1
                            height: parent.height
                            color: Theme.surface2
                        }
                        Column {
                            x: rel.nested ? 52 : 40
                            anchors.verticalCenter: parent.verticalCenter
                            width: actions.x - x - 12
                            spacing: 2
                            Row {
                                spacing: 8
                                Text {
                                    text: rel.modelData.version
                                    color: Theme.text
                                    font { family: Theme.mono; pointSize: Theme.pt(rel.nested ? 11.5 : 12.5); weight: Font.Medium }
                                }
                                Badge { text: Up.releaseFacts(rel.modelData, page.now) }
                            }
                            Text {
                                width: parent.width
                                visible: !rel.expanded && (rel.modelData.changes ?? []).length > 0
                                text: (rel.modelData.changes ?? [])[0] ?? ""
                                elide: Text.ElideRight
                                color: Theme.muted
                                font { family: Theme.font; pointSize: Theme.pt(11) }
                            }
                        }
                        MouseArea {
                            id: rowMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: rel.expanded = !rel.expanded
                        }
                        Row {
                            id: actions
                            anchors { right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
                            spacing: 8
                            Text {
                                visible: rel.action[0] !== ""
                                anchors.verticalCenter: parent.verticalCenter
                                text: rel.action[0]
                                color: rel.modelData.state === "running" ? Theme.rose : Theme.muted
                                font { family: Theme.font; pointSize: Theme.pt(12); weight: Font.Medium }
                            }
                            Button {
                                visible: rel.action[1] !== ""
                                text: rel.action[1]
                                label: rel.action[1] + " " + rel.modelData.version
                                kind: rel.modelData.state === "installed" ? "plain" : "ghost"
                                enabled: !page.waiting
                                onClicked: page.choose(rel.modelData)
                            }
                        }
                    }
                    // What's in it (opened by clicking the row).
                    Column {
                        visible: rel.expanded
                        x: rel.nested ? 52 : 40
                        width: parent.width - x - 20
                        bottomPadding: 14
                        spacing: 4
                        Repeater {
                            model: rel.modelData.changes ?? []
                            delegate: Text {
                                required property string modelData
                                width: parent.width
                                wrapMode: Text.Wrap
                                text: "•  " + modelData
                                color: Theme.muted
                                font { family: Theme.font; pointSize: Theme.pt(11.5) }
                            }
                        }
                        Text {
                            visible: (rel.modelData.changes ?? []).length === 0
                            text: "No notes for this release."
                            color: Theme.muted
                            font { family: Theme.font; pointSize: Theme.pt(11.5) }
                        }
                    }
                }
            }
        }
    }

    // Going to an older release: what that means, before it happens.
    Dialog {
        id: confirmOlder
        property var release: null
        readonly property string version: release?.version ?? ""
        strip: true
        icon: "alert"
        title: "Switch to WhiteRose " + confirmOlder.version + "?"
        subtitle: "It's older than " + Up.newestRelease(page.series) + ". Some features may be missing or not work properly. Issues reported from a version that isn't the newest are closed automatically."
        footer: [
            Button { text: "Cancel"; onClicked: confirmOlder.close() },
            Button {
                text: (confirmOlder.release?.state === "installed" ? "Switch to " : "Install ") + confirmOlder.version
                kind: "primary"
                onClicked: {
                    const r = confirmOlder.release;
                    confirmOlder.close();
                    page.install(r);
                }
            }
        ]
    }
}
