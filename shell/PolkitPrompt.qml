// The password prompt when something needs your permission (polkit: adding
// a user, changing a system setting, an app asking for admin rights). The
// shell is this session's polkit agent. A card over a dimmed screen, like
// the lock screen; Enter confirms, Esc cancels. A wrong password shakes the
// card and lets you try again.
//
// Nested inside another desktop, that desktop stays the agent (only one per
// session), so this registers only when WhiteRose is the session.
import QtQuick
import Quickshell
import Quickshell.Wayland
import Quickshell.Widgets
import Quickshell.Services.Polkit

Scope {
    id: root

    PolkitAgent {
        id: agent
        onIsRegisteredChanged: if (isRegistered) console.info("polkit: WhiteRose is this session's agent")
    }
    readonly property var flow: agent.flow
    // PAM is waiting for a finger (pam_fprintd: it talks, it doesn't ask).
    readonly property bool fingerprint: flow !== null && !flow.isResponseRequired
        && /finger|swipe/i.test(flow.supplementaryMessage ?? "")

    PanelWindow {
        visible: root.flow !== null && !ShellState.locked
        screen: Quickshell.screens.find(s => s.name === (Compositor.outputs.find(o => o.enabled)?.connector ?? "")) ?? Quickshell.screens[0] ?? null
        anchors { top: true; bottom: true; left: true; right: true }
        exclusionMode: ExclusionMode.Ignore
        color: Qt.rgba(0, 0, 0, 0.5)
        WlrLayershell.layer: WlrLayer.Overlay
        WlrLayershell.namespace: "whiterose-polkit"
        LayerTitle { name: "whiterose-polkit" }
        WlrLayershell.keyboardFocus: WlrKeyboardFocus.Exclusive

        // A click outside the card doesn't dismiss it (it's a question).
        MouseArea { anchors.fill: parent }

        Rectangle {
            id: card
            anchors.centerIn: parent
            width: 420
            height: body.implicitHeight + 40
            radius: 22
            color: Theme.bg
            border { width: 1; color: Theme.surface2 }

            SequentialAnimation {
                id: shake
                loops: 1
                NumberAnimation { target: card; property: "anchors.horizontalCenterOffset"; to: -10; duration: 50 }
                NumberAnimation { target: card; property: "anchors.horizontalCenterOffset"; to: 10; duration: 70 }
                NumberAnimation { target: card; property: "anchors.horizontalCenterOffset"; to: -6; duration: 60 }
                NumberAnimation { target: card; property: "anchors.horizontalCenterOffset"; to: 0; duration: 50 }
            }
            Connections {
                target: root.flow
                function onAuthenticationFailed() { shake.restart(); field.text = ""; card.busy = false }
                function onIsResponseRequiredChanged() { if (root.flow?.isResponseRequired) { card.busy = false; field.forceActiveFocus() } }
            }
            property bool busy: false

            Column {
                id: body
                x: 22; y: 20
                width: parent.width - 44
                spacing: 14

                Row {
                    spacing: 12
                    Rectangle {
                        width: 40; height: 40; radius: 12
                        color: Theme.rose
                        Icon { anchors.centerIn: parent; name: "lock"; size: 18; color: Theme.onAccent }
                    }
                    Column {
                        anchors.verticalCenter: parent.verticalCenter
                        spacing: 2
                        Text { text: "Authentication required"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(15); weight: Font.DemiBold } }
                        Text {
                            text: root.flow?.selectedIdentity ? "As " + (root.flow.selectedIdentity.displayName || root.flow.selectedIdentity.string) : ""
                            color: Theme.muted
                            font { family: Theme.font; pointSize: Theme.pt(11.5) }
                        }
                    }
                }
                Text {
                    width: parent.width
                    text: root.flow?.message ?? ""
                    wrapMode: Text.Wrap
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(12.5) }
                }
                // Who to authenticate as, when there's a choice (several admins).
                Row {
                    visible: (root.flow?.identities.length ?? 0) > 1
                    spacing: 6
                    Repeater {
                        model: root.flow?.identities ?? []
                        delegate: Rectangle {
                            required property var modelData
                            readonly property bool on: root.flow?.selectedIdentity === modelData
                            width: idText.implicitWidth + 20; height: 26; radius: 13
                            color: on ? Theme.pearl : Theme.surface1
                            Text { id: idText; anchors.centerIn: parent; text: modelData.displayName || modelData.string; color: parent.on ? Theme.bg : Theme.text; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
                            MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: root.flow.selectedIdentity = modelData }
                        }
                    }
                }
                Rectangle {
                    width: parent.width; height: 40; radius: 12
                    color: Theme.surface1
                    border { width: 1; color: field.activeFocus ? Theme.muted : Theme.surface2 }
                    TextInput {
                        id: field
                        anchors { fill: parent; leftMargin: 14; rightMargin: 14 }
                        verticalAlignment: TextInput.AlignVCenter
                        echoMode: root.flow?.responseVisible ? TextInput.Normal : TextInput.Password
                        passwordCharacter: "•"
                        color: Theme.text
                        selectionColor: Theme.rose
                        enabled: root.flow?.isResponseRequired ?? false
                        font { family: Theme.font; pointSize: Theme.pt(13) }
                        focus: true
                        onAccepted: card.submit()
                        Keys.onEscapePressed: card.cancel()
                        Text {
                            anchors.verticalCenter: parent.verticalCenter
                            visible: field.text === ""
                            text: root.fingerprint ? "Waiting for the reader…" : (root.flow?.inputPrompt ?? "Password").replace(/:\s*$/, "")
                            color: Theme.faint
                            font: field.font
                        }
                    }
                }
                // What PAM says: a wrong password, or (with fingerprints on for
                // admin prompts, Settings › Users) the reader listening.
                Row {
                    visible: note.text !== ""
                    width: parent.width
                    spacing: 8
                    Icon {
                        id: noteIcon
                        visible: root.fingerprint
                        anchors.verticalCenter: parent.verticalCenter
                        name: "fingerprint"; size: 16
                        color: root.flow?.supplementaryIsError ? Theme.rose : Theme.muted
                        SequentialAnimation on opacity {
                            running: root.fingerprint && !(root.flow?.supplementaryIsError ?? false)
                            loops: Animation.Infinite
                            onRunningChanged: if (!running) noteIcon.opacity = 1
                            NumberAnimation { to: 0.45; duration: 900; easing.type: Easing.InOutSine }
                            NumberAnimation { to: 1; duration: 900; easing.type: Easing.InOutSine }
                        }
                    }
                    Text {
                        id: note
                        width: parent.width - (noteIcon.visible ? noteIcon.width + parent.spacing : 0)
                        wrapMode: Text.Wrap
                        text: root.fingerprint && !(root.flow?.supplementaryIsError ?? false)
                            ? "Touch the fingerprint reader, or wait to type your password"
                            : (root.flow?.supplementaryMessage ?? "")
                        color: root.flow?.supplementaryIsError ? Theme.rose : Theme.muted
                        font { family: Theme.font; pointSize: Theme.pt(11.5) }
                    }
                }
                Row {
                    anchors.right: parent.right
                    spacing: 8
                    PromptButton { text: "Cancel"; onClicked: card.cancel() }
                    PromptButton { text: card.busy ? "Checking…" : "Authenticate"; primary: true; enabled: field.text !== "" && !card.busy; onClicked: card.submit() }
                }
            }

            function submit() {
                if (!root.flow || field.text === "" || busy) return;
                busy = true;
                root.flow.submit(field.text);
                field.text = "";
            }
            function cancel() {
                busy = false;
                field.text = "";
                root.flow?.cancelAuthenticationRequest();
            }
        }
    }

    component PromptButton: Rectangle {
        property string text
        property bool primary: false
        property bool enabled: true
        signal clicked()
        width: label.implicitWidth + 28; height: 34; radius: 10
        opacity: enabled ? 1 : 0.45
        color: primary ? Theme.pearl : mouse.containsMouse ? Theme.grip : Theme.surface2
        Text { id: label; anchors.centerIn: parent; text: parent.text; color: parent.primary ? Theme.bg : Theme.text; font { family: Theme.font; pointSize: Theme.pt(12.5); weight: Font.Medium } }
        MouseArea { id: mouse; anchors.fill: parent; enabled: parent.enabled; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: parent.clicked() }
    }
}
