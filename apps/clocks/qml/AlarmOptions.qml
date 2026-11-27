import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Clocks

Column {
    id: root
    property var value
    property var sounds: []
    signal edited(var options)
    signal importRequested()
    spacing: 14
    function change(key, value) {
        Clock.stopPreview();
        const next = Object.assign({}, root.value);
        next[key] = value;
        if (key === "ring_duration" && value > 0) next.fade_duration = Math.min(next.fade_duration, value);
        root.edited(next);
    }
    component Label: Text {
        color: Theme.muted
        font { family: Theme.font; pointSize: Theme.pt(12) }
        textFormat: Text.PlainText
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }
    component Picker: Choice {
        width: root.width; height: 34
        anchors.verticalCenter: undefined
    }
    Label { text: "Sound"; color: Theme.text; font.weight: Font.Medium }
    Picker {
        label: "Alarm chime"
        options: root.sounds.map(sound => ({label: sound.name, value: sound.id}))
        value: root.value.sound
        onPicked: value => root.change("sound", value)
    }
    Row {
        spacing: 8
        Action {
            text: Clock.previewing ? "Stop preview" : "Preview"
            icon: Clock.previewing ? "pause" : "play"
            enabled: Clock.connected && !Clock.busy
            onClicked: { if (Clock.previewing) Clock.stopPreview(); else Clock.preview(JSON.stringify(root.value)) }
        }
        Action { text: "Import audio"; icon: "folder"; onClicked: { Clock.stopPreview(); root.importRequested() } }
    }
    Label { width: parent.width; text: "Custom audio is copied locally (up to 20 MB, first 60 seconds). Preview plays up to 8 seconds."; wrapMode: Text.Wrap; font.pointSize: Theme.pt(11) }
    Item {
        width: parent.width; height: 22
        Label { text: "Volume"; color: Theme.text }
        Label { anchors.right: parent.right; text: root.value.volume + "%"; color: Theme.text; font.family: Theme.mono }
    }
    Slider {
        id: level
        width: parent.width; height: 22
        from: 0; to: 100; stepSize: 5
        value: root.value.volume
        wheelEnabled: false
        Accessible.name: "Alarm volume"
        onMoved: root.change("volume", Math.round(value))
        background: Rectangle {
            x: level.leftPadding; y: level.topPadding + (level.availableHeight-height)/2
            width: level.availableWidth; height: 5; radius: 3; color: Theme.surface2
            Rectangle { width: parent.width * level.visualPosition; height: parent.height; radius: 3; color: Theme.pearl }
        }
        handle: Rectangle {
            x: level.leftPadding + level.visualPosition * (level.availableWidth-width)
            y: level.topPadding + (level.availableHeight-height)/2
            width: 14; height: 14; radius: 7; color: Theme.pearl
            border { color: level.activeFocus ? Theme.rose : Theme.surface1; width: 2 }
        }
    }
    Label { text: root.value.volume === 0 ? "Silent · the notification still appears" : "Uses the current output; system volume still applies"; font.pointSize: Theme.pt(11) }
    Label { text: "Snooze"; color: Theme.text; font.weight: Font.Medium }
    Picker {
        label: "Snooze duration"
        options: [1,2,5,10,15,30,60].map(n => ({label:n + (n === 1 ? " minute" : " minutes"),value:n*60000}))
        value: root.value.snooze_duration
        onPicked: value => root.change("snooze_duration", value)
    }
    Label { text: "Stop ringing after"; color: Theme.text; font.weight: Font.Medium }
    Picker {
        label: "Ring duration"
        options: [{label:"Until dismissed",value:0},{label:"30 seconds",value:30000},{label:"1 minute",value:60000},{label:"2 minutes",value:120000},{label:"5 minutes",value:300000},{label:"10 minutes",value:600000},{label:"30 minutes",value:1800000},{label:"1 hour",value:3600000}]
        value: root.value.ring_duration
        onPicked: value => root.change("ring_duration", value)
    }
    Label { width: parent.width; text: "After the sound stops, the alert stays available to snooze or dismiss."; wrapMode: Text.Wrap; font.pointSize: Theme.pt(11) }
    Label { text: "Gradual volume"; color: Theme.text; font.weight: Font.Medium }
    Picker {
        label: "Fade-in duration"
        options: [0,5000,10000,30000,60000].map(ms => ({label: ms ? "Fade in over " + ms/1000 + " seconds" : "Off",value:ms,disabled:root.value.ring_duration>0 && ms>root.value.ring_duration}))
        value: root.value.fade_duration
        onPicked: value => root.change("fade_duration", value)
    }
}
