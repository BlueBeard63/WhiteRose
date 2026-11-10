// Sound: where sound goes and comes from (PipeWire), their volumes, and each
// app that's playing. The places come from the daemon ("audio_ports"): it
// also lists ones in another setup of the sound card (a laptop's Speaker
// when the Headphones setup is on) and marks unplugged ones, which can't be
// picked. Picking one switches the card's setup first if needed.
import QtQuick
import Quickshell
import Quickshell.Services.Pipewire
import qs
import qs.settings

Column {
    id: page
    spacing: 16

    readonly property var nodes: Pipewire.nodes.values.filter(n => n.audio)
    readonly property var apps: nodes.filter(n => n.isStream && n.isSink)
    readonly property var sink: Pipewire.defaultAudioSink
    readonly property var source: Pipewire.defaultAudioSource

    // [{ key, label, direction, available, node }] from the daemon.
    property var ports: []
    property string error: ""
    property string switching: ""
    function load() {
        Daemon.request({ op: "audio_ports" }, (v, err) => { if (!err && v) page.ports = v });
    }
    // Asked again whenever PipeWire's devices change (a setup switched,
    // something plugged in), a moment later so the change has settled.
    Timer { id: reload; interval: 300; onTriggered: page.load() }
    readonly property int nodeCount: Pipewire.nodes.values.length
    onNodeCountChanged: reload.restart()
    Component.onCompleted: load()

    function portOptions(dir) {
        // Unplugged ones last.
        const ps = page.ports.filter(p => p.direction === dir);
        return ps.filter(p => p.available !== "no").concat(ps.filter(p => p.available === "no")).map(p => ({
            label: p.label, value: p.key,
            hint: p.available === "no" ? "Unplugged" : "",
            disabled: p.available === "no",
        }));
    }
    function current(dir, node) {
        const p = page.ports.find(p => p.direction === dir && node && p.node === node.name);
        return p ? p.key : "";
    }
    function use(key) {
        page.error = "";
        page.switching = key;
        Daemon.request({ op: "audio_use", key: key }, (v, err) => {
            page.switching = "";
            if (err) page.error = String(err);
            page.load();
        });
    }

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
            subtitle: page.error
            Choice {
                enabled: page.switching === ""
                value: page.current("out", page.sink)
                options: page.portOptions("out")
                onPicked: k => page.use(k)
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
                enabled: page.switching === "" && page.ports.some(p => p.direction === "in")
                value: page.current("in", page.source)
                options: page.portOptions("in")
                onPicked: k => page.use(k)
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
