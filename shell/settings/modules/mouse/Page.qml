// Mouse & Touchpad: scroll direction, pointer speed and acceleration for
// each, and tap to click. The compositor applies them through libinput to
// every mouse and touchpad, including ones plugged in later. (In a window
// on another desktop, that desktop's own settings apply.)
import QtQuick
import qs
import qs.settings

Column {
    id: page
    spacing: 16

    PageHeader { title: "Mouse & Touchpad"; subtitle: "How the pointer moves and which way things scroll." }

    Section { text: "Mouse" }
    Card {
        ScrollRow { prefix: "mouse"; natural: false }
        SpeedRow { prefix: "mouse" }
        SettingRow {
            title: "Acceleration"
            subtitle: "Faster movements carry the pointer further"
            Toggle { checked: Daemon.setting("mouse.acceleration", true); onToggled: on => Daemon.set("mouse.acceleration", on) }
        }
    }

    Section { text: "Touchpad" }
    Card {
        SettingRow {
            divider: false
            title: "Tap to click"
            subtitle: "A tap is a click; two fingers, a right click"
            Toggle { checked: Daemon.setting("touchpad.tapToClick", true); onToggled: on => Daemon.set("touchpad.tapToClick", on) }
        }
        ScrollRow { prefix: "touchpad"; natural: true }
        SpeedRow { prefix: "touchpad" }
        SettingRow {
            title: "Acceleration"
            subtitle: "Faster movements carry the pointer further"
            Toggle { checked: Daemon.setting("touchpad.acceleration", true); onToggled: on => Daemon.set("touchpad.acceleration", on) }
        }
    }

    // Which way content moves (`natural`: this kind's default).
    component ScrollRow: SettingRow {
        id: scroll
        required property string prefix
        required property bool natural
        readonly property bool on: Daemon.setting(prefix + ".naturalScroll", natural)
        divider: prefix !== "mouse"
        title: "Scrolling"
        subtitle: on ? "Natural: the content follows your fingers, as on a phone" : "Traditional: the page moves the way the scroll bar does"
        Segmented {
            value: scroll.on
            options: [{ label: "Traditional", value: false }, { label: "Natural", value: true }]
            onPicked: v => Daemon.set(scroll.prefix + ".naturalScroll", v)
        }
    }

    // libinput's speed, -1 to 1: the middle is the device's own.
    component SpeedRow: SettingRow {
        id: speed
        required property string prefix
        readonly property real value: Daemon.setting(prefix + ".speed", 0)
        property real shown: value
        onValueChanged: shown = value
        title: "Pointer speed"
        Slider {
            implicitWidth: 200
            value: speed.shown + 1; maximum: 2
            // (Near the middle: the default, exactly.)
            onMoved: v => { const s = Math.round((v - 1) * 20) / 20; speed.shown = Math.abs(s) < 0.06 ? 0 : s; save.restart() }
        }
        Text {
            width: 64; horizontalAlignment: Text.AlignRight; anchors.verticalCenter: parent.verticalCenter
            text: speed.shown === 0 ? "Default" : (speed.shown > 0 ? "+" : "−") + Math.round(Math.abs(speed.shown) * 100) + "%"
            color: Theme.muted; font { family: Theme.mono; pointSize: Theme.pt(11.5) }
        }
        Timer { id: save; interval: 250; onTriggered: Daemon.set(speed.prefix + ".speed", speed.shown) }
    }
}
