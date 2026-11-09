// Sound: where sound goes and comes from (PipeWire), their volumes, and each
// app that's playing.
import QtQuick
import Quickshell
import Quickshell.Services.Pipewire
import qs
import qs.settings

Column {
    id: page
    spacing: 16

    readonly property var nodes: Pipewire.nodes.values.filter(n => n.audio)
    readonly property var outputs: nodes.filter(n => n.isSink && !n.isStream)
    readonly property var inputs: nodes.filter(n => !n.isSink && !n.isStream && !(n.name ?? "").endsWith(".monitor"))
    readonly property var apps: nodes.filter(n => n.isStream && n.isSink)
    readonly property var sink: Pipewire.defaultAudioSink
    readonly property var source: Pipewire.defaultAudioSource

    // Keeps every shown node's volume live.
    PwObjectTracker { objects: page.nodes }

    function label(n) { return n?.nickname || n?.description || n?.name || "" }
    function appName(n) { return n.properties["application.name"] || n.properties["media.name"] || label(n) }
    function pct(v) { return Math.round(v * 100) + "%" }

    PageHeader { title: "Sound"; subtitle: "Outputs, inputs and per-app volume, through PipeWire." }

    Section { text: "Output" }
    Card {
        SettingRow {
            divider: false
            title: "Play sound through"
            Choice {
                value: page.sink
                options: page.outputs.map(n => ({ label: page.label(n), value: n }))
                onPicked: n => Pipewire.preferredDefaultAudioSink = n
            }
        }
        SettingRow {
            visible: page.sink !== null
            title: "Volume"
            IconButton {
                width: 28; height: 28; iconSize: 14
                icon: page.sink?.audio?.muted ? "volumeMuted" : "volume"
                iconColor: page.sink?.audio?.muted ? Theme.rose : Theme.text
                onClicked: page.sink.audio.muted = !page.sink.audio.muted
            }
            Slider {
                value: page.sink?.audio?.volume ?? 0
                maximum: 1.5
                dimmed: page.sink?.audio?.muted ?? false
                onMoved: v => { page.sink.audio.volume = v; page.sink.audio.muted = false }
            }
            Text { width: 40; text: page.pct(page.sink?.audio?.volume ?? 0); color: Theme.muted; horizontalAlignment: Text.AlignRight; anchors.verticalCenter: parent.verticalCenter; font { family: Theme.mono; pointSize: Theme.pt(11) } }
        }
    }

    Section { text: "Input" }
    Card {
        SettingRow {
            divider: false
            title: "Microphone"
            Choice {
                enabled: page.inputs.length > 0
                value: page.source
                options: page.inputs.map(n => ({ label: page.label(n), value: n }))
                onPicked: n => Pipewire.preferredDefaultAudioSource = n
            }
        }
        SettingRow {
            visible: page.source !== null
            title: "Input level"
            IconButton {
                width: 28; height: 28; iconSize: 14
                icon: "mic"
                iconColor: page.source?.audio?.muted ? Theme.rose : Theme.text
                onClicked: page.source.audio.muted = !page.source.audio.muted
            }
            Slider {
                value: page.source?.audio?.volume ?? 0
                maximum: 1.5
                dimmed: page.source?.audio?.muted ?? false
                onMoved: v => { page.source.audio.volume = v; page.source.audio.muted = false }
            }
            Text { width: 40; text: page.pct(page.source?.audio?.volume ?? 0); color: Theme.muted; horizontalAlignment: Text.AlignRight; anchors.verticalCenter: parent.verticalCenter; font { family: Theme.mono; pointSize: Theme.pt(11) } }
        }
    }

    Section { text: "Apps"; visible: page.apps.length > 0 }
    Card {
        visible: page.apps.length > 0
        Repeater {
            model: page.apps
            delegate: SettingRow {
                required property var modelData
                required property int index
                divider: index > 0
                title: page.appName(modelData)
                subtitle: modelData.properties["media.name"] && modelData.properties["application.name"] ? modelData.properties["media.name"] : ""
                IconButton {
                    width: 28; height: 28; iconSize: 14
                    icon: modelData.audio.muted ? "volumeMuted" : "volume"
                    iconColor: modelData.audio.muted ? Theme.rose : Theme.text
                    onClicked: modelData.audio.muted = !modelData.audio.muted
                }
                Slider {
                    value: modelData.audio.volume
                    dimmed: modelData.audio.muted
                    onMoved: v => { modelData.audio.volume = v; modelData.audio.muted = false }
                }
                Text { width: 40; text: page.pct(modelData.audio.volume); color: Theme.muted; horizontalAlignment: Text.AlignRight; anchors.verticalCenter: parent.verticalCenter; font { family: Theme.mono; pointSize: Theme.pt(11) } }
            }
        }
    }
}
