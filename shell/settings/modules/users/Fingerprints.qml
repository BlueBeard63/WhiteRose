// Users › Fingerprints: the fingers you've added (fprintd, through
// whiterosed's fingerprint ops), adding one (the reader asks for a number of
// touches) and removing them, and where a finger works: the lock screen
// always; sign-in, sudo and admin prompts when switched on for this computer.
// Adding and removing ask for your password first (fprintd's rule).
import QtQuick
import QtQuick.Controls
import QtQuick.Shapes
import qs
import qs.settings
import "Users.js" as U

Column {
    id: page
    spacing: 16

    property var info: null
    property string loadError: ""
    property string error: ""
    property bool busy: false
    readonly property bool hasReader: (info?.reader ?? "") !== ""
    readonly property var fingers: info?.fingers ?? []

    function refresh() {
        Daemon.request({ op: "fingerprints" }, (v, err) => {
            page.loadError = err ?? "";
            if (!err) page.info = v;
        });
    }
    Component.onCompleted: refresh()

    PageHeader {
        title: "Fingerprints"
        subtitle: "Users › unlock with a finger instead of typing your password."
        Button {
            visible: page.hasReader && U.nextFinger(page.fingers) !== ""
            text: "Add a fingerprint"; icon: "plus"; kind: "primary"
            onClicked: addDialog.openNew()
        }
    }

    SettingRow {
        visible: page.loadError !== ""
        title: "Fingerprints aren't available"
        subtitle: page.loadError
    }
    Card {
        visible: page.info !== null && !page.hasReader
        SettingRow {
            divider: false
            title: "No fingerprint reader"
            subtitle: "Plug one in, or use a computer that has one built in."
        }
    }

    // ---- yours -----------------------------------------------------------------------
    Section { visible: page.hasReader; text: "Your fingerprints · " + (page.info?.reader ?? "") }
    Card {
        visible: page.hasReader
        SettingRow {
            visible: page.fingers.length === 0
            divider: false
            title: "None added yet"
            subtitle: "Add one to unlock with a touch."
        }
        Repeater {
            model: page.fingers
            delegate: SettingRow {
                required property string modelData
                required property int index
                divider: index > 0
                title: U.fingerName(modelData)
                subtitle: "Unlocks this computer"
                Button {
                    text: "Remove"; kind: "ghost"
                    enabled: !page.busy
                    onClicked: {
                        page.busy = true;
                        page.error = "";
                        Daemon.request({ op: "fingerprint_delete", args: { finger: modelData } }, (v, err) => {
                            page.busy = false;
                            page.error = err ?? "";
                            if (v) page.info = v; else page.refresh();
                        });
                    }
                }
            }
        }
    }
    Text {
        visible: page.error !== ""
        width: parent.width
        wrapMode: Text.Wrap
        text: page.error
        color: Theme.rose
        font { family: Theme.font; pointSize: Theme.pt(11.5) }
    }

    // ---- where -------------------------------------------------------------------------
    Section { visible: page.hasReader; text: "Where it works" }
    Card {
        visible: page.hasReader
        SettingRow {
            title: "Lock screen"
            subtitle: "Touch the reader instead of typing your password"
            Badge { dot: false; text: page.fingers.length > 0 ? "on" : "add a finger first" }
        }
        SettingRow {
            divider: true
            title: "Sign-in, sudo and admin prompts"
            subtitle: !(page.info?.login?.supported ?? false)
                ? "This system's sign-in can't be changed here."
                : "For everyone on this computer: the login screen, sudo in a terminal and password prompts ask for a finger first, then the password. Needs an administrator."
            dimmed: !(page.info?.login?.supported ?? false)
            Toggle {
                id: loginToggle
                checked: page.info?.login?.enabled ?? false
                enabled: (page.info?.login?.supported ?? false) && !page.busy
                onToggled: on => {
                    page.busy = true;
                    page.error = "";
                    Daemon.request({ op: "fingerprint_login", on: on }, (v, err) => {
                        page.busy = false;
                        page.error = err ?? "";
                        if (v) page.info = v; else page.refresh();
                        // Put the switch back to what's true.
                        loginToggle.checked = Qt.binding(() => page.info?.login?.enabled ?? false);
                    });
                }
            }
        }
    }

    // ---- adding one ----------------------------------------------------------------------
    Dialog {
        id: addDialog
        icon: "fingerprint"
        title: running ? U.fingerName(finger) : finished ? "Fingerprint added" : "Add a fingerprint"
        subtitle: running || finished ? "" : "Pick the finger, then touch the reader when asked"
        closePolicy: running ? Popup.NoAutoClose : (Popup.CloseOnEscape | Popup.CloseOnPressOutside)

        property string finger: ""
        property bool running: false
        property bool finished: false
        property var last: null
        property string failure: ""

        function openNew() {
            finger = U.nextFinger(page.fingers);
            running = finished = false;
            last = null;
            failure = "";
            open();
        }
        function start() {
            running = true;
            failure = "";
            last = null;
            Daemon.request({ op: "fingerprint_enroll", args: { finger: finger } }, (v, err) => {
                addDialog.running = false;
                if (v) page.info = v; else page.refresh();
                if (err) addDialog.failure = err;
                else if (addDialog.last?.state !== "cancelled") addDialog.finished = true;
            });
        }
        onClosed: if (running) Daemon.request({ op: "fingerprint_cancel" })

        Connections {
            target: Daemon
            function onFingerprintEvent(e) { if (e.finger === addDialog.finger) addDialog.last = e }
        }

        // Which finger.
        Flow {
            visible: !addDialog.running && !addDialog.finished
            width: parent.width
            spacing: 6
            Repeater {
                model: U.fingers
                delegate: Rectangle {
                    required property string modelData
                    readonly property bool taken: page.fingers.includes(modelData)
                    readonly property bool on: addDialog.finger === modelData
                    width: label.implicitWidth + 22; height: 30; radius: 15
                    opacity: taken ? 0.4 : 1
                    color: on ? Theme.pearl : mouse.containsMouse && !taken ? Theme.grip : Theme.surface1
                    Text {
                        id: label
                        anchors.centerIn: parent
                        text: U.fingerName(modelData) + (parent.taken ? " ✓" : "")
                        color: parent.on ? Theme.bg : Theme.text
                        font { family: Theme.font; pointSize: Theme.pt(11.5) }
                    }
                    MouseArea {
                        id: mouse
                        anchors.fill: parent
                        hoverEnabled: true
                        enabled: !parent.taken
                        cursorShape: Qt.PointingHandCursor
                        onClicked: addDialog.finger = modelData
                    }
                }
            }
        }

        // The reader: a ring that fills with each touch.
        Item {
            visible: addDialog.running || addDialog.finished
            width: parent.width; height: 200
            Shape {
                id: ring
                anchors.centerIn: parent
                width: 140; height: 140
                readonly property real fraction: addDialog.finished ? 1 : U.enrollFraction(addDialog.last)
                preferredRendererType: Shape.CurveRenderer
                ShapePath {
                    strokeColor: Theme.surface2; strokeWidth: 6; fillColor: "transparent"
                    PathAngleArc { centerX: 70; centerY: 70; radiusX: 64; radiusY: 64; startAngle: 0; sweepAngle: 360 }
                }
                ShapePath {
                    strokeColor: addDialog.last?.state === "retry" ? Theme.muted : Theme.rose
                    strokeWidth: 6; fillColor: "transparent"; capStyle: ShapePath.RoundCap
                    PathAngleArc {
                        centerX: 70; centerY: 70; radiusX: 64; radiusY: 64; startAngle: -90
                        sweepAngle: 360 * ring.fraction
                        Behavior on sweepAngle { NumberAnimation { duration: 250; easing.type: Easing.OutCubic } }
                    }
                }
                Icon {
                    id: bigPrint
                    anchors.centerIn: parent
                    name: addDialog.finished ? "check" : "fingerprint"
                    size: 56
                    color: addDialog.finished || ring.fraction > 0 ? Theme.rose : Theme.muted
                    SequentialAnimation on opacity {
                        running: addDialog.running
                        loops: Animation.Infinite
                        onRunningChanged: if (!running) bigPrint.opacity = 1
                        NumberAnimation { to: 0.5; duration: 900; easing.type: Easing.InOutSine }
                        NumberAnimation { to: 1; duration: 900; easing.type: Easing.InOutSine }
                    }
                }
            }
            Text {
                anchors { horizontalCenter: parent.horizontalCenter; top: ring.bottom; topMargin: 14 }
                text: addDialog.finished ? "Touch the reader on the lock screen to unlock."
                    : (addDialog.last?.stage ?? 0) === 0 && addDialog.last?.state !== "retry"
                        ? U.enrollText(addDialog.last, page.info?.scan ?? "press") + " (it may ask for your password first)"
                        : U.enrollText(addDialog.last, page.info?.scan ?? "press")
                color: addDialog.last?.state === "retry" ? Theme.rose : Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(12) }
            }
        }

        Text {
            visible: addDialog.failure !== ""
            width: parent.width
            wrapMode: Text.Wrap
            text: addDialog.failure
            color: Theme.rose
            font { family: Theme.font; pointSize: Theme.pt(11.5) }
        }

        width: 480
        footer: [
            Item { width: parent.width - fpClose.width - (fpStart.visible ? fpStart.width + 8 : 0) - 8; height: 1 },
            Button {
                id: fpClose
                text: addDialog.finished ? "Done" : "Cancel"
                kind: addDialog.finished ? "primary" : "plain"
                onClicked: addDialog.close()
            },
            Button {
                id: fpStart
                visible: !addDialog.running && !addDialog.finished
                text: addDialog.failure !== "" ? "Try again" : "Start"
                kind: "primary"
                enabled: addDialog.finger !== ""
                onClicked: addDialog.start()
            }
        ]
    }
}
