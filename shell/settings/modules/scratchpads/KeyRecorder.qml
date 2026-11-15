// A scratchpad's key (Figma: "Settings - Scratchpads", the Key row): its
// caps; click to record a new one, as on the Keyboard page. A key another
// shortcut uses waits for Enter (taking it from that one) or Escape;
// `clash` says which, for the row to show. Sends `recorded(combo)`.
import QtQuick
import qs
import qs.settings
import "../keyboard/Keys.js" as K

Item {
    id: rec
    required property var keys
    // This pad's shortcut id, so its own key isn't a clash.
    required property string shortcutId
    property string label: "Key"
    signal recorded(string combo)

    property bool recording: false
    property string held: ""
    property string captured: ""
    property string problem: ""
    readonly property var clash: captured ? K.conflict(Compositor.shortcuts, captured, shortcutId) : null

    implicitWidth: row.implicitWidth + 16
    implicitHeight: 30
    anchors.verticalCenter: parent?.verticalCenter

    Accessible.role: Accessible.Button
    Accessible.name: recording ? "Recording a key for " + label : label + ": " + (keys.length > 0 ? keys.map(K.spoken).join(", ") : "none")
    Accessible.onPressAction: start()

    function start() {
        recording = true;
        held = "";
        captured = "";
        problem = "";
        Compositor.pauseShortcuts(true);
        catcher.forceActiveFocus();
    }
    function stop() {
        if (!recording) return;
        recording = false;
        held = "";
        captured = "";
        problem = "";
        Compositor.pauseShortcuts(false);
    }
    function finish(c) {
        captured = K.normalize(c);
        if (!clash) commit();
    }
    function commit() {
        const c = captured;
        stop();
        rec.recorded(c);
    }
    Component.onDestruction: if (recording) Compositor.pauseShortcuts(false)

    Rectangle {
        anchors.fill: parent
        radius: 8
        color: rec.recording ? "transparent" : mouse.containsMouse ? Theme.surface2 : "transparent"
        border { width: rec.recording ? 1 : 0; color: Theme.pearl }
    }
    Row {
        id: row
        anchors.centerIn: parent
        spacing: 5
        Rectangle { visible: rec.recording; width: 6; height: 6; radius: 3; color: Theme.rose; anchors.verticalCenter: parent.verticalCenter }
        Repeater {
            model: rec.recording ? K.caps(rec.captured || rec.held) : (rec.keys.length > 0 ? K.caps(rec.keys[0]) : [])
            delegate: KeyCap { required property string modelData; text: modelData }
        }
        Text {
            visible: (rec.recording && rec.captured === "" && rec.held === "") || (!rec.recording && rec.keys.length === 0)
            anchors.verticalCenter: parent.verticalCenter
            text: rec.recording ? "Press a key…" : "Set a key"
            color: rec.recording ? Theme.muted : Theme.faint
            font { family: Theme.font; pointSize: Theme.pt(11.5) }
        }
    }
    MouseArea {
        id: mouse
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: rec.recording ? rec.stop() : rec.start()
    }

    // Takes the keys while recording.
    Item {
        id: catcher
        function mods(e) {
            return { logo: (e.modifiers & Qt.MetaModifier) !== 0, ctrl: (e.modifiers & Qt.ControlModifier) !== 0,
                     alt: (e.modifiers & Qt.AltModifier) !== 0, shift: (e.modifiers & Qt.ShiftModifier) !== 0 };
        }
        Keys.onPressed: e => {
            e.accepted = true;
            if (!rec.recording) return;
            if (rec.captured !== "") {
                if (e.key === Qt.Key_Return || e.key === Qt.Key_Enter) rec.commit();
                else if (e.key === Qt.Key_Escape) rec.stop();
                return;
            }
            const m = mods(e);
            if (K.isModifier(e.key)) {
                if (K.isSuper(e.key)) m.logo = true;
                rec.held = K.combo(m, "");
                return;
            }
            if (e.key === Qt.Key_Escape && !m.logo && !m.ctrl && !m.alt && !m.shift) { rec.stop(); return; }
            const name = K.keyName(e.key, e.nativeScanCode);
            if (name === "") { rec.problem = "That key can't be a shortcut"; return; }
            rec.problem = "";
            rec.finish(K.combo(m, name));
        }
        Keys.onReleased: e => {
            e.accepted = true;
            if (rec.recording && rec.captured === "" && K.isModifier(e.key)) rec.held = K.combo(mods(e), "");
        }
        onActiveFocusChanged: if (!activeFocus) rec.stop()
    }
}
