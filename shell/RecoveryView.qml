// Island view (Figma: Recovery - Update Failed): a new WhiteRose version
// didn't start, so the session launcher went back to the one before and put
// your settings back. Try again restarts the session on the new version (two
// more tries); Stay keeps this one (the new one isn't tried again until a
// newer release); View log opens what it printed. ShellState opens this once
// per session while a failure is recorded.
import QtQuick
import Quickshell

Item {
    id: root
    implicitWidth: 540
    implicitHeight: body.implicitHeight + 40

    readonly property var f: ShellState.updateFailure
    readonly property bool gaveUp: (f?.attempts ?? 0) >= (f?.tries ?? 2)
    property bool busy: false
    property string error: ""

    function act(op) {
        busy = true;
        error = "";
        Daemon.request({ op: op }, (v, err) => {
            if (!root) return;
            busy = false;
            error = err ?? "";
            if (v) ShellState.noteUpdates(v);
        });
    }
    function tildify(p) {
        const home = Quickshell.env("HOME") ?? "";
        return home && p.startsWith(home + "/") ? "~" + p.slice(home.length) : p;
    }

    Column {
        id: body
        x: 24; y: 20
        width: parent.width - 48
        spacing: 16

        Row {
            width: parent.width
            spacing: 14
            Rectangle {
                width: 42; height: 42; radius: 12
                color: Theme.tint(0.14)
                Icon { anchors.centerIn: parent; name: "alert"; size: 19; color: Theme.rose }
            }
            Column {
                width: parent.width - 56
                spacing: 4
                Text {
                    text: "WhiteRose " + (root.f?.version ?? "") + " couldn't start"
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(14.5); weight: Font.DemiBold }
                }
                Text {
                    width: parent.width
                    wrapMode: Text.Wrap
                    text: (root.gaveUp
                            ? "It failed to start twice, so you're back on " + (root.f?.back_on ?? "") + "."
                            : "It didn't start, so you're back on " + (root.f?.back_on ?? "") + ". It'll be tried once more at your next login.")
                        + " Your settings were restored from the snapshot taken before the update — nothing was lost."
                    color: Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(11.5) }
                }
            }
        }

        // What went wrong, and where the log is.
        Rectangle {
            width: parent.width
            height: details.implicitHeight + 24
            radius: 10
            color: Theme.surface1
            Column {
                id: details
                x: 12; y: 12
                width: parent.width - 24
                spacing: 4
                Text {
                    width: parent.width
                    elide: Text.ElideRight
                    text: "attempt " + (root.f?.attempts ?? 0) + "/" + (root.f?.tries ?? 2) + " · " + (root.f?.reason ?? "")
                    color: Theme.muted
                    font { family: Theme.mono; pointSize: Theme.pt(10.5) }
                }
                Text {
                    width: parent.width
                    elide: Text.ElideMiddle
                    text: "log  " + root.tildify(root.f?.log ?? "")
                    color: Theme.faint
                    font { family: Theme.mono; pointSize: Theme.pt(10.5) }
                }
            }
        }

        Text {
            visible: root.error !== ""
            width: parent.width
            wrapMode: Text.Wrap
            text: root.error
            color: Theme.rose
            font { family: Theme.font; pointSize: Theme.pt(11.5) }
        }

        Item {
            width: parent.width; height: 32
            // View log: plain text, so it opens in the text editor.
            Rectangle {
                width: logText.implicitWidth + 28; height: 32; radius: 9
                color: logMouse.containsMouse ? Theme.surface1 : "transparent"
                Text { id: logText; anchors.centerIn: parent; text: "View log"; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(12.5) } }
                MouseArea {
                    id: logMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    cursorShape: Qt.PointingHandCursor
                    onClicked: Quickshell.execDetached(["xdg-open", root.f?.log ?? ""])
                }
            }
            Row {
                anchors.right: parent.right
                spacing: 8
                opacity: root.busy ? 0.5 : 1
                ToastButton {
                    width: implicitWidth
                    implicitWidth: 120
                    icon: "refresh"; label: "Try again"
                    onClicked: if (!root.busy) root.act("update_try_again")
                }
                Rectangle {
                    width: stayRow.implicitWidth + 28; height: 32; radius: 9
                    color: stayMouse.containsMouse ? Qt.lighter(Theme.pearl, 1.04) : Theme.pearl
                    Row {
                        id: stayRow
                        anchors.centerIn: parent
                        spacing: 6
                        Icon { anchors.verticalCenter: parent.verticalCenter; name: "check"; size: 14; color: Theme.bg }
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            text: "Stay on " + (root.f?.back_on ?? "")
                            color: Theme.bg
                            font { family: Theme.font; pointSize: Theme.pt(12.5); weight: Font.Medium }
                        }
                    }
                    MouseArea {
                        id: stayMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: if (!root.busy) root.act("update_stay")
                    }
                }
            }
        }
    }
}
