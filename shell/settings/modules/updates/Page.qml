// Updates (Figma: Settings - Updates): the version you're on, a new release
// with what's in it and how far it's got (downloaded, signature, files
// verified, installed alongside, switch on restart), switching to it (at the
// next login, or by restarting the session now), going back to the previous
// version, and how updates behave. whiterosed does the work (internal/updater);
// installing asks for an administrator's password.
import QtQuick
import Quickshell
import qs
import qs.settings
import "Updates.js" as Up

Column {
    id: page
    spacing: 16

    property var st: null
    property string error: ""
    property bool waiting: false   // a request in flight (buttons wait)
    // (For "checked 5 minutes ago".)
    property real now: Date.now()
    Timer { interval: 30000; running: true; repeat: true; onTriggered: page.now = Date.now() }

    readonly property var rel: st?.release ?? null

    function show(v) {
        page.st = v;
        SettingsState.updatesBadge(v);
    }
    function refresh() {
        Daemon.request({ op: "updates" }, (v, err) => {
            if (!page) return;
            page.error = err ?? "";
            if (v) page.show(v);
        });
    }
    // Runs an update op; the status comes back with the answer (and as events).
    function act(op, extra) {
        page.waiting = true;
        page.error = "";
        const req = Object.assign({ op: op }, extra ?? {});
        Daemon.request(req, (v, err) => {
            if (!page) return;
            page.waiting = false;
            page.error = err ?? "";
            if (v) page.show(v); else page.refresh();
        });
    }
    Component.onCompleted: refresh()
    Connections {
        target: Daemon
        function onUpdatesEvent(v) { page.show(v) }
    }

    PageHeader {
        title: "Updates"
        subtitle: Up.headline(page.st, page.now)
        Button {
            visible: page.st?.supported ?? false
            text: page.st?.busy === "checking" ? "Checking…" : "Check now"
            icon: "refresh"
            enabled: !page.st?.busy && !page.waiting
            onClicked: page.act("update_check")
        }
    }

    // Not for this system / build.
    Card {
        visible: page.st !== null && !page.st.supported
        SettingRow { divider: false; title: "Updates aren't available here"; subtitle: page.st?.why ?? "" }
    }

    // What went wrong last (a check, a download, an install).
    Text {
        readonly property string msg: page.error || (page.st?.error ?? "")
        visible: msg !== ""
        width: parent.width
        wrapMode: Text.Wrap
        text: msg
        color: Theme.rose
        font { family: Theme.font; pointSize: Theme.pt(11.5) }
    }

    // A new version didn't start (the island showed it too).
    Card {
        visible: !!page.st?.failure
        SettingRow {
            divider: false
            title: "WhiteRose " + (page.st?.failure?.version ?? "") + " couldn't start"
            subtitle: "Back on " + (page.st?.failure?.back_on ?? "") + " · attempt " + (page.st?.failure?.attempts ?? 0) + "/" + (page.st?.failure?.tries ?? 2) + " · " + (page.st?.failure?.reason ?? "")
            Button { text: "View log"; kind: "ghost"; onClicked: Quickshell.execDetached(["xdg-open", page.st.failure.log]) }
            Button { text: "Try again"; icon: "refresh"; enabled: !page.waiting; onClicked: page.act("update_try_again") }
            Button { text: "Stay on " + (page.st?.failure?.back_on ?? ""); icon: "check"; kind: "primary"; enabled: !page.waiting; onClicked: page.act("update_stay") }
        }
    }

    // Up to date.
    Card {
        visible: (page.st?.supported ?? false) && page.rel === null
        Item {
            width: parent.width; height: 64
            Rectangle {
                id: okBadge
                x: 18; anchors.verticalCenter: parent.verticalCenter
                width: 32; height: 32; radius: 10
                color: Theme.surface2
                Icon { anchors.centerIn: parent; name: "check"; size: 16; color: Theme.rose }
            }
            Column {
                anchors { left: okBadge.right; leftMargin: 12; verticalCenter: parent.verticalCenter }
                spacing: 2
                Text { text: "You're up to date"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.Medium } }
                Text {
                    text: (page.st?.checked ?? 0) ? "WhiteRose " + page.st.running + " is the newest release" : "Not checked yet"
                    color: Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(11.5) }
                }
            }
        }
    }

    // ---- the new release --------------------------------------------------------------
    Card {
        visible: page.rel !== null
        Item {
            width: parent.width
            height: rc.implicitHeight + 40
            Column {
                id: rc
                x: 20; y: 20
                width: parent.width - 40
                spacing: 14

                Item {
                    width: parent.width; height: 26
                    Row {
                        spacing: 10
                        anchors.verticalCenter: parent.verticalCenter
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: "WhiteRose " + (page.rel?.version ?? "")
                            color: Theme.text
                            font { family: Theme.font; pointSize: Theme.pt(15); weight: Font.DemiBold }
                        }
                        // "✓ Ready to apply" (Figma), or where it's got to.
                        Rectangle {
                            readonly property var b: Up.badge(page.rel)
                            visible: b[0] !== ""
                            anchors.verticalCenter: parent.verticalCenter
                            width: bRow.implicitWidth + 16; height: 22; radius: 6
                            color: b[1] === "ready" ? Qt.rgba(Theme.rose.r, Theme.rose.g, Theme.rose.b, 0.16) : Theme.surface2
                            Row {
                                id: bRow
                                anchors.centerIn: parent
                                spacing: 5
                                Icon { visible: parent.parent.b[1] === "ready"; anchors.verticalCenter: parent.verticalCenter; name: "check"; size: 11; color: Theme.rose }
                                Text {
                                    text: parent.parent.b[0]
                                    color: parent.parent.b[1] === "ready" ? Theme.rose : Theme.muted
                                    font { family: Theme.font; pointSize: Theme.pt(11) }
                                }
                            }
                        }
                    }
                    Text {
                        anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                        text: (page.rel?.size ?? "") + (page.rel?.published ? " · released " + Up.day(page.rel.published, page.now) : "")
                        color: Theme.faint
                        font { family: Theme.font; pointSize: Theme.pt(11) }
                    }
                }

                // What's in it.
                Column {
                    visible: (page.rel?.changes ?? []).length > 0
                    width: parent.width
                    spacing: 4
                    Repeater {
                        model: page.rel?.changes ?? []
                        delegate: Row {
                            required property string modelData
                            spacing: 8
                            Text { text: "•"; color: Theme.faint; font { family: Theme.font; pointSize: Theme.pt(12) } }
                            Text { text: modelData; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(12) } }
                        }
                    }
                }

                // How far it's got.
                Row {
                    id: steps
                    width: parent.width
                    readonly property var list: page.rel?.steps ?? []
                    Repeater {
                        model: steps.list
                        delegate: Item {
                            id: step
                            required property var modelData
                            required property int index
                            width: steps.width / Math.max(1, steps.list.length)
                            height: 70
                            // The line on to the next step (lit once this one's done).
                            Rectangle {
                                visible: step.index < steps.list.length - 1
                                x: 26; y: 10
                                width: step.width - 30; height: 2; radius: 1
                                color: step.modelData.done ? Theme.rose : Theme.surface2
                            }
                            Rectangle {
                                id: dot
                                width: 22; height: 22; radius: 11
                                color: step.modelData.done ? Theme.rose : "transparent"
                                border { width: step.modelData.done ? 0 : 2; color: step.modelData.active ? Theme.text : Theme.surface2 }
                                Icon { visible: step.modelData.done; anchors.centerIn: parent; name: "check"; size: 12; color: Theme.onAccent }
                                Rectangle {
                                    visible: step.modelData.active && !step.modelData.done
                                    anchors.centerIn: parent
                                    width: 8; height: 8; radius: 4
                                    color: Theme.text
                                }
                            }
                            Column {
                                y: 32
                                width: step.width - 8
                                spacing: 3
                                Text {
                                    text: step.modelData.name
                                    color: step.modelData.done || step.modelData.active ? Theme.text : Theme.muted
                                    font { family: Theme.font; pointSize: Theme.pt(12); weight: Font.Medium }
                                }
                                Text {
                                    width: parent.width
                                    elide: Text.ElideRight
                                    text: step.modelData.detail
                                    color: Theme.faint
                                    font { family: Theme.font; pointSize: Theme.pt(10.5) }
                                }
                            }
                        }
                    }
                }

                // Downloading: how much.
                Rectangle {
                    visible: page.rel?.state === "downloading"
                    width: parent.width; height: 4; radius: 2
                    color: Theme.surface2
                    Rectangle {
                        width: parent.width * Up.fraction(page.st); height: parent.height; radius: 2
                        color: Theme.rose
                        Behavior on width { NumberAnimation { duration: 200 } }
                    }
                }

                Item {
                    width: parent.width; height: 32
                    Text {
                        anchors { left: parent.left; right: actions.left; rightMargin: 12; verticalCenter: parent.verticalCenter }
                        elide: Text.ElideRight
                        text: Up.note(page.rel, page.st ?? {})
                        color: Theme.faint
                        font { family: Theme.font; pointSize: Theme.pt(11) }
                    }
                    Row {
                        id: actions
                        anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                        spacing: 8
                        readonly property string state: page.rel?.state ?? ""
                        readonly property bool free: !page.st?.busy && !page.waiting
                        Button {
                            visible: actions.state === "available" || actions.state === "downloading"
                            text: actions.state === "downloading" ? "Downloading…" : "Download"
                            kind: "primary"
                            enabled: actions.free
                            onClicked: page.act("update_download")
                        }
                        Button {
                            visible: actions.state === "downloaded" || actions.state === "installing"
                            text: actions.state === "installing" ? "Installing…" : page.waiting ? "Waiting for permission…" : "Install"
                            kind: "primary"
                            enabled: actions.free
                            onClicked: page.act("update_install")
                        }
                        Button {
                            visible: actions.state === "ready"
                            text: page.rel?.pending ? "Applies at next login" : "Apply at next login"
                            icon: page.rel?.pending ? "check" : ""
                            enabled: actions.free && !page.rel?.pending
                            onClicked: page.act("update_apply")
                        }
                        Button {
                            visible: actions.state === "ready" && (page.st?.launcher ?? false)
                            text: "Restart session now"
                            icon: "restart"
                            kind: "primary"
                            enabled: actions.free
                            onClicked: confirmRestart.open()
                        }
                    }
                }
            }
        }
    }

    // ---- how updates behave -------------------------------------------------------------
    Card {
        visible: page.st?.supported ?? false
        SettingRow {
            visible: (page.st?.previous ?? "") !== "" || (page.st?.rolled_to ?? "") !== ""
            divider: false
            title: "Previous version"
            subtitle: page.st?.rolled_to
                ? "Going back to " + page.st.rolled_to + " at the next login; " + page.st.running + " won't be tried again until a newer release"
                : "Kept on disk to go back to, until the next update replaces it"
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: page.st?.rolled_to || page.st?.previous || ""
                color: Theme.muted
                font { family: Theme.mono; pointSize: Theme.pt(11.5) }
            }
            Button {
                visible: !page.st?.rolled_to
                text: "Roll back"
                enabled: !page.waiting
                onClicked: page.act("update_rollback")
            }
            Button {
                visible: !!page.st?.rolled_to
                text: "Stay on " + (page.st?.running ?? "")
                enabled: !page.waiting
                onClicked: page.act("update_undo_rollback")
            }
            Button {
                visible: !!page.st?.rolled_to && (page.st?.launcher ?? false)
                text: "Restart now"; icon: "restart"; kind: "primary"
                enabled: !page.waiting
                onClicked: confirmRestart.open()
            }
        }
        SettingRow {
            divider: (page.st?.previous ?? "") !== "" || (page.st?.rolled_to ?? "") !== ""
            title: "Automatic rollback"
            subtitle: "If a new version fails to start twice, return to the previous one"
            Toggle {
                id: rollbackToggle
                checked: page.st?.auto_rollback ?? true
                enabled: !page.waiting
                onToggled: on => {
                    page.act("update_auto_rollback", { on: on });
                    rollbackToggle.checked = Qt.binding(() => page.st?.auto_rollback ?? true);
                }
            }
        }
        SettingRow {
            title: "Settings snapshot"
            subtitle: "WhiteRose copies your settings before a new version first starts, and puts them back if it fails"
            Badge { dot: false; text: page.st?.snapshot ? "Taken " + Up.ago(page.st.snapshot, page.now) : "None yet" }
        }
        SettingRow {
            title: "Check for updates"
            subtitle: "No downloads on metered connections"
            Choice {
                value: Daemon.setting("update.check", "daily")
                options: [{ label: "Daily", value: "daily" }, { label: "Weekly", value: "weekly" }, { label: "Manually", value: "manual" }]
                onPicked: v => Daemon.set("update.check", v)
            }
        }
    }

    Dialog {
        id: confirmRestart
        strip: true
        icon: "restart"
        title: "Restart the session?"
        subtitle: "Your apps will close. Save your work first."
        Text {
            width: parent.width
            wrapMode: Text.Wrap
            text: page.st?.rolled_to
                ? "WhiteRose " + page.st.rolled_to + " starts straight away."
                : "WhiteRose " + (page.rel?.version ?? "") + " starts straight away. If it doesn't come up properly, " + (page.st?.running ?? "") + " starts instead and your settings are put back."
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(12) }
        }
        footer: [
            Button { id: rcCancel; text: "Cancel"; onClicked: confirmRestart.close() },
            Button {
                id: rcGo
                text: "Restart now"; kind: "primary"
                onClicked: {
                    confirmRestart.close();
                    if (!page.st?.rolled_to && page.rel && !page.rel.pending) {
                        // Make sure it's the one that starts.
                        Daemon.request({ op: "update_apply" }, () => page.act("update_restart"));
                    } else {
                        page.act("update_restart");
                    }
                }
            }
        ]
    }
}
