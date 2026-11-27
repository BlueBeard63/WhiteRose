// Approved Task manager review: Figma 217:1705. Native kit, live Linux data.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Tasks
import "Tasks.js" as Format

AppWindow {
    id: win
    title: "Task manager"; visible: true
    width: 960; height: 720
    minimumWidth: 520; minimumHeight: 440
    property var data: ({})
    readonly property var system: data.system || {}
    readonly property var apps: system.apps || []
    readonly property var protection: data.protection || {}
    readonly property bool wide: width >= 820
    property bool paused: false
    property string page: "Apps & processes"
    property string metric: "CPU"
    property string device: ""
    property string sortBy: "memory"
    property bool onlyApps: true
    property string selectedUnit: ""
    property var expanded: ({})
    readonly property var selected: apps.find(a => a.unit === selectedUnit) || null
    property var target: null
    property string dialog: ""
    readonly property bool canAct: Tasks.connected && !Tasks.busy && !paused && selected !== null && selected.can_end
    readonly property var filtered: Format.rows(apps, search.text, sortBy, onlyApps)
    readonly property var series: metric === "CPU" ? (system.cpu || []).slice(0,1)
        : metric === "Memory" ? (system.memory && system.memory.total ? [system.memory] : [])
        : metric === "Storage" ? system.disks || [] : metric === "Network" ? system.network || [] : system.gpu || []
    readonly property var current: series.find(s => s.id === device) || series[0] || {history:[],other_history:[]}
    readonly property bool throughput: metric === "Storage" || metric === "Network"
    readonly property real graphMax: throughput ? Math.max(1024, ...current.history.filter(v => v !== null), ...current.other_history.filter(v => v !== null)) * 1.15 : 100
    FontLoader { id: doto; source: "qrc:/qt/qml/WhiteRose/Tasks/fonts/Doto.ttf" }
    toolbar: Text { text: "Task manager"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.Medium } }
    onPausedChanged: if (!paused) win.data = JSON.parse(Tasks.state)
    onMetricChanged: device = ""
    onDialogChanged: if (dialog) Qt.callLater(() => cancel.forceActiveFocus())
    Connections {
        target: Tasks
        function onStateChanged() { if (!win.paused) win.data = JSON.parse(Tasks.state) }
        function onConnectedChanged() { if (!Tasks.connected && win.dialog !== "details") win.dialog = "" }
        function onCompleted() { if (!Tasks.error) win.dialog = "" }
    }
    Timer { interval: 1000; running: win.visible; repeat: true; onTriggered: Tasks.refresh() }
    function send(op,args) { if (Tasks.connected && !Tasks.busy && !paused) Tasks.command(op,JSON.stringify(args)) }
    function kept(a) { return (protection.apps || []).find(p => p.unit === a.unit) || {} }
    function protect(id,on) { send("tasks_protect",{id:id,on:on}) }
    function confirm(kind) {
        if (!canAct) return;
        target = {unit:selected.unit,token:selected.token,id:selected.id,count:selected.processes.length};
        dialog = kind;
    }
    readonly property bool targetValid: target !== null && apps.some(a => a.unit === target.unit && a.token === target.token && a.can_end)

    component Label: Text {
        color: Theme.text; textFormat: Text.PlainText
        font { family: Theme.font; pointSize: Theme.pt(13) }
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }
    component Note: Label { color: Theme.muted; wrapMode: Text.Wrap; width: parent.width; font.pointSize: Theme.pt(12) }
    component Card: Rectangle { radius: 12; color: Theme.surface1 }
    component Nav: Action {
        property bool chosen: false
        color: chosen ? Theme.grip : "transparent"
        opacity: chosen ? 1 : 0.7
        Accessible.role: Accessible.RadioButton
        Accessible.checkable: true
        Accessible.checked: chosen
    }
    Item {
        anchors.fill: parent
        Flickable {
            id: scroll
            anchors.fill: parent; clip: true
            enabled: !win.dialog
            contentHeight: content.implicitHeight + 40
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            Column {
                id: content
                x: 20; y: 20; width: scroll.width - 40; spacing: 10
                Row {
                    spacing: 4
                    Repeater {
                        model: [{name:"Apps & processes",icon:"appWindow"},{name:"Performance",icon:"gauge"},{name:"Memory protection",icon:"shield"}]
                        delegate: Nav {
                            required property var modelData
                            text: modelData.name; icon: modelData.icon
                            chosen: win.page === text
                            onClicked: win.page = text
                        }
                    }
                }
                Item {
                    width: parent.width; height: 38
                    Label { text: win.page; font { pointSize: Theme.pt(22); weight: Font.Medium } anchors.verticalCenter: parent.verticalCenter }
                    Action {
                        anchors.right: parent.right; anchors.verticalCenter: parent.verticalCenter
                        text: win.paused ? "Resume" : "Pause"; icon: win.paused ? "play" : "pause"
                        onClicked: win.paused = !win.paused
                    }
                }
                Note {
                    visible: !Tasks.connected || win.paused || Tasks.error !== ""
                    text: Tasks.error || (!Tasks.connected ? "Reconnecting… Last snapshot is frozen. Actions are disabled." : "Updates paused. Resume before changing or ending an app.")
                    color: Tasks.error ? Theme.rose : Theme.muted
                }
                Row {
                    visible: win.page === "Apps & processes"
                    width: parent.width; spacing: 10
                    Repeater {
                        model: [
                            {label:"CPU",value:Format.percent((win.system.cpu || [])[0]?.value)},
                            {label:"Memory",value:Format.size(win.system.memory?.used)},
                            {label:"Network · interfaces",value:Format.rate((win.system.network || []).filter(n => n.id !== "lo").reduce((n,s) => s.value === null ? n : (n === null ? 0 : n) + s.value, null))}
                        ]
                        delegate: Card {
                            required property var modelData
                            width: (content.width - 20) / 3; height: 58
                            Column {
                                x: 14; y: 8; spacing: 3; width: parent.width - 28
                                Label { text: modelData.label; color: Theme.muted; font.pointSize: Theme.pt(11) }
                                Label { text: modelData.value; width: parent.width; elide: Text.ElideRight; font { family: doto.name; pointSize: Theme.pt(21); weight: Font.Bold } }
                            }
                        }
                    }
                }
                Row {
                    visible: win.page === "Apps & processes"
                    width: parent.width; spacing: 8; height: 36
                    Field { id: search; width: parent.width - sorter.width - scope.width - 16; placeholder: "Search apps, processes or PID"; label: "Search apps, processes or PID"; icon: "search" }
                    Choice {
                        id: scope; anchors.verticalCenter: undefined
                        label: "Process filter"; value: win.onlyApps
                        options: [{label:"Apps",value:true},{label:"All processes",value:false}]
                        onPicked: v => win.onlyApps = v
                    }
                    Choice {
                        id: sorter; anchors.verticalCenter: undefined
                        label: "Sort apps"; value: win.sortBy
                        options: [{label:"Sort: memory",value:"memory"},{label:"Sort: CPU",value:"cpu"},{label:"Sort: read",value:"read"},{label:"Sort: write",value:"write"},{label:"Sort: name",value:"name"}]
                        onPicked: v => win.sortBy = v
                    }
                }
                Card {
                    visible: win.page === "Apps & processes"
                    width: parent.width; height: Math.max(190, win.height - 360)
                    Column {
                        anchors.fill: parent; anchors.margins: 10; spacing: 4
                        Row {
                            visible: win.wide; width: parent.width; height: 24
                            Label { text: "App / process"; width: parent.width * 0.40; color: Theme.muted; font.pointSize: Theme.pt(11) }
                            Repeater {
                                model: ["CPU", "Own memory", "Disk read / write", "Network"]
                                delegate: Label { required property string modelData; text: modelData; width: (appList.width * 0.60) / 4; color: Theme.muted; font.pointSize: Theme.pt(11) }
                            }
                        }
                        ListView {
                            id: appList
                            width: parent.width; height: parent.height - (win.wide ? 28 : 0)
                            clip: true; model: win.filtered; spacing: 2
                            ScrollBar.vertical: ScrollBar {}
                            Label { visible: appList.count === 0; anchors.centerIn: parent; text: search.text ? "No matching apps or processes" : !win.system.time ? "Connecting…" : !win.system.scoped && win.onlyApps ? "App scopes unavailable. Choose All processes." : "No apps running"; color: Theme.muted }
                            delegate: Column {
                                id: appRow
                                required property var modelData
                                width: appList.width
                                readonly property bool open: !!win.expanded[modelData.unit]
                                Rectangle {
                                    width: parent.width; height: win.wide ? 44 : 64; radius: 8
                                    color: win.selectedUnit === appRow.modelData.unit ? Theme.tint(0.12) : rowMouse.containsMouse ? Theme.surface2 : "transparent"
                                    border { width: win.selectedUnit === appRow.modelData.unit ? 1 : 0; color: Theme.tint(0.35) }
                                    Accessible.role: Accessible.ListItem
                                    Accessible.name: Format.name(appRow.modelData.id) + " app"
                                    Accessible.description: appRow.modelData.reason || Format.size(appRow.modelData.memory)
                                    Accessible.onPressAction: win.selectedUnit = appRow.modelData.unit
                                    MouseArea { id: rowMouse; anchors.fill: parent; hoverEnabled: true; onClicked: win.selectedUnit = appRow.modelData.unit; onDoubleClicked: { win.selectedUnit = appRow.modelData.unit; win.dialog = "details" } }
                                    Row {
                                        x: 8; y: win.wide ? 7 : 8; spacing: 6; width: parent.width * (win.wide ? 0.40 : 1) - 16
                                        Action {
                                            text: ""; icon: appRow.open ? "chevDown" : "chevR"; width: 22; height: 28; kind: "ghost"
                                            Accessible.name: "Expand " + Format.name(appRow.modelData.id)
                                            onClicked: { const e = Object.assign({},win.expanded); e[appRow.modelData.unit] = !appRow.open; win.expanded = e; win.selectedUnit = appRow.modelData.unit }
                                        }
                                        Icon { name: win.kept(appRow.modelData).kept ? "shield" : "appWindow"; size: 16; anchors.verticalCenter: parent.verticalCenter }
                                        Label {
                                            text: Format.name(appRow.modelData.id) + " · " + appRow.modelData.processes.length + " processes"
                                            width: parent.width - 56; elide: Text.ElideRight
                                            anchors.verticalCenter: parent.verticalCenter
                                        }
                                    }
                                    Row {
                                        visible: win.wide
                                        x: parent.width * 0.40; y: 7; width: parent.width * 0.60
                                        Repeater {
                                            model: [Format.percent(appRow.modelData.cpu),Format.size(appRow.modelData.memory),Format.rate(appRow.modelData.read) + "\n" + Format.rate(appRow.modelData.write),"Unavailable"]
                                            delegate: Label { required property string modelData; text: modelData; width: appList.width * 0.60 / 4; font { family: Theme.mono; pointSize: Theme.pt(11) } color: Theme.muted }
                                        }
                                    }
                                    Label {
                                        visible: !win.wide; x: 42; y: 38; width: parent.width - 54
                                        text: Format.percent(appRow.modelData.cpu) + " CPU · " + Format.size(appRow.modelData.memory) + " memory" + (win.kept(appRow.modelData).kept ? " · Protected" : "")
                                        color: Theme.muted; font.pointSize: Theme.pt(11); elide: Text.ElideRight
                                    }
                                }
                                Repeater {
                                    model: appRow.open ? appRow.modelData.processes : []
                                    delegate: Label {
                                        required property var modelData
                                        width: appList.width - 42; x: 42; height: 28; verticalAlignment: Text.AlignVCenter
                                        text: modelData.pid + "  " + modelData.name + " · " + modelData.state + " · " + Format.percent(modelData.cpu) + " · " + Format.size(modelData.memory)
                                        font { family: Theme.mono; pointSize: Theme.pt(11) } color: Theme.muted; elide: Text.ElideRight
                                    }
                                }
                            }
                        }
                    }
                }
                Row {
                    visible: win.page === "Apps & processes"
                    spacing: 8
                    Action { text: "Details"; icon: "info"; enabled: win.selected !== null; onClicked: win.dialog = "details" }
                    Action { text: "End"; icon: "square"; enabled: win.canAct; onClicked: win.confirm("end") }
                    Action { text: "Force end"; icon: "x"; kind: "danger"; enabled: win.canAct; onClicked: win.confirm("force") }
                    Action { text: "Protect"; icon: "shield"; enabled: Tasks.connected && !Tasks.busy && !win.paused && win.selected !== null && !win.selected.unit.startsWith("pid:"); onClicked: win.protect(win.selected.id,true) }
                }
                Note {
                    visible: win.page === "Apps & processes"
                    text: (win.selected?.reason ? win.selected.reason + ". " : "") + "CPU: all logical CPUs together = 100%. Own memory excludes cache/shared pages. Per-app network is unavailable."
                }
                // Performance: persistent selectors, one-minute history and per-core cards.
                Row {
                    visible: win.page === "Performance"
                    width: parent.width; spacing: 16
                    Column {
                        width: win.wide ? 156 : 108; spacing: 4
                        Repeater {
                            model: [{name:"CPU",icon:"cpu"},{name:"Memory",icon:"memory"},{name:"Storage",icon:"drive"},{name:"Network",icon:"ethernet"},{name:"GPU",icon:"monitor"}]
                            delegate: Nav {
                                required property var modelData
                                text: modelData.name; icon: modelData.icon
                                width: parent.width; height: 42; chosen: win.metric === text
                                onClicked: win.metric = text
                            }
                        }
                    }
                    Column {
                        width: parent.width - (win.wide ? 172 : 124); spacing: 12
                        Row {
                            width: parent.width; spacing: 12
                            Label { text: win.metric; font { pointSize: Theme.pt(22); weight: Font.Medium } }
                            Choice {
                                visible: win.series.length > 1
                                label: win.metric + " device"; value: win.current.id
                                options: win.series.map(s => ({label:s.id,value:s.id}))
                                onPicked: v => win.device = v
                            }
                        }
                        Card {
                            width: parent.width; height: 260
                            Column {
                                x: 16; y: 14; width: parent.width - 32; spacing: 8
                                Label {
                                    text: win.throughput ? Format.rate(win.current.value) : Format.percent(win.current.value)
                                    font { family: doto.name; pointSize: Theme.pt(30); weight: Font.Bold }
                                }
                                Note { text: win.throughput ? (win.metric === "Storage" ? "Read · " : "Receive · ") + Format.rate(win.current.value) + (win.metric === "Storage" ? "   Write · " : "   Send · ") + Format.rate(win.current.other) : win.metric === "Memory" ? Format.size(win.current.used) + " used of " + Format.size(win.current.total) : win.current.id || "No supported device" }
                                Graph { width: parent.width; height: 136; values: win.current.history; other: win.throughput ? win.current.other_history : []; ceiling: win.graphMax }
                                Row {
                                    width: parent.width
                                    Label { width: parent.width / 2; text: "60 seconds"; color: Theme.muted; font.pointSize: Theme.pt(11) }
                                    Label { width: parent.width / 2; horizontalAlignment: Text.AlignRight; text: "Now"; color: Theme.muted; font.pointSize: Theme.pt(11) }
                                }
                            }
                        }
                        Grid {
                            visible: win.metric === "CPU"
                            columns: win.wide ? 4 : 2; spacing: 8; width: parent.width
                            Repeater {
                                model: (win.system.cpu || []).slice(1)
                                delegate: Card {
                                    required property var modelData
                                    width: (parent.width - 8 * (parent.columns - 1)) / parent.columns; height: 72
                                    Label { x: 10; y: 8; text: modelData.id.toUpperCase() + " · " + Format.percent(modelData.value); font.pointSize: Theme.pt(11) }
                                    Graph { x: 10; y: 32; width: parent.width - 20; height: 30; values: modelData.history }
                                }
                            }
                        }
                        Note {
                            text: win.metric === "CPU" ? "Utilization is normalized across all logical CPUs. Each core graph uses its own 100% scale."
                                : win.metric === "Memory" ? "System use includes kernel memory and shared pages; it is not the sum of app own-memory readings."
                                : win.metric === "Network" ? "Interface totals, not app traffic. Per-app attribution is unavailable. Loopback is shown separately."
                                : win.metric === "Storage" ? "Whole-device read/write throughput. Device-mapper and physical devices can overlap; they are never summed."
                                : "GPU activity and dedicated memory depend on driver support. Missing counters are unavailable, not zero."
                        }
                        Note { visible: win.metric === "GPU"; text: "Dedicated memory: " + Format.size(win.current.used) + " / " + Format.size(win.current.total) + "\nPer-app GPU attribution: Unavailable" }
                        Note { visible: win.current.value === null || win.current.value === undefined; text: "Unavailable: waiting for a second sample or a supported counter." }
                    }
                }
                // Memory protection uses the watcher's settings and judgments.
                Column {
                    visible: win.page === "Memory protection"
                    width: parent.width; spacing: 12
                    Card {
                        width: parent.width; height: 76
                        Column {
                            x: 16; y: 14; spacing: 8; width: parent.width - 32
                            Label { text: "Memory · " + (win.protection.level || "Checking…"); font.pointSize: Theme.pt(17) }
                            Note { text: Format.size(win.system.memory?.used) + " in use · " + Format.size(win.protection.available) + " available" }
                        }
                    }
                    Card {
                        width: parent.width; height: 80
                        Column {
                            x: 16; y: 14; width: parent.width - 80; spacing: 6
                            Label { text: "Close apps when memory runs out" }
                            Note { text: "Automatic closure only. Protected apps stay running; explicit End is a separate action." }
                        }
                        Toggle {
                            anchors.right: parent.right; anchors.rightMargin: 18
                            label: "Close apps when memory runs out"; checked: !!win.data.auto_close
                            enabled: Tasks.connected && !Tasks.busy && !win.paused
                            onToggled: on => win.send("set",{on:on})
                        }
                    }
                    Label { text: "Apps running"; font.weight: Font.Medium }
                    Repeater {
                        model: win.protection.apps || []
                        delegate: Card {
                            required property var modelData
                            width: content.width; height: 58
                            Column {
                                x: 16; y: 9; width: parent.width - 80; spacing: 4
                                Label { text: Format.name(modelData.id); width: parent.width; elide: Text.ElideRight }
                                Note { text: Format.size(modelData.bytes) + (modelData.kept === "terminal" ? " · Terminal · always protected" : modelData.suspect ? " · At risk · first to close" : modelData.growing ? " · Growing steadily" : modelData.kept ? " · Protected" : " · Automatic protection"); color: modelData.suspect ? Theme.rose : Theme.muted }
                            }
                            Toggle {
                                anchors.right: parent.right; anchors.rightMargin: 18
                                label: "Keep " + Format.name(modelData.id) + " running"; checked: !!modelData.kept
                                enabled: Tasks.connected && !Tasks.busy && !win.paused && modelData.kept !== "terminal"
                                onToggled: on => win.protect(modelData.id,on)
                            }
                        }
                    }
                    Note { visible: !(win.protection.apps || []).length; text: "No app scopes available. Automatic memory protection needs app scopes." }
                    Repeater {
                        model: (win.data.protected || []).filter(id => !(win.protection.apps || []).some(a => a.id.toLowerCase() === id.toLowerCase()))
                        delegate: Card {
                            required property string modelData
                            width: content.width; height: 48
                            Label { x: 16; anchors.verticalCenter: parent.verticalCenter; text: Format.name(modelData) + " · Not open" }
                            Toggle { anchors.right: parent.right; anchors.rightMargin: 18; checked: true; label: "Keep " + Format.name(modelData) + " running"; enabled: Tasks.connected && !Tasks.busy && !win.paused; onToggled: on => win.protect(modelData,on) }
                        }
                    }
                    Label { text: "Recently closed to free memory"; font.weight: Font.Medium }
                    Repeater {
                        model: (win.protection.closed || []).slice(0,5)
                        delegate: Note { required property var modelData; text: Format.name(modelData.app) + " · " + Qt.formatDateTime(new Date(modelData.time),"d MMM, hh:mm") }
                    }
                    Note { visible: !(win.protection.closed || []).length; text: "No apps closed automatically." }
                }
            }
        }
        Rectangle {
            anchors.fill: parent; visible: !!win.dialog; color: "#99000000"
            MouseArea { anchors.fill: parent; onClicked: {} }
            Card {
                anchors.centerIn: parent
                width: Math.min(480,parent.width - 40); height: Math.min(dialogContent.implicitHeight + 40,parent.height - 24)
                Flickable {
                    anchors.fill: parent; clip: true; contentHeight: dialogContent.implicitHeight + 40
                    ScrollBar.vertical: ScrollBar {}
                    Column {
                        id: dialogContent
                        x: 20; y: 20; width: parent.width - 40; spacing: 16
                        Row {
                            spacing: 10
                            Icon { name: win.dialog === "details" ? "info" : win.dialog === "force" ? "x" : "square"; size: 22; color: win.dialog === "force" ? Theme.rose : Theme.text }
                            Label { text: win.dialog === "details" ? "App details" : win.dialog === "force" ? "Force end this app?" : "End this app?"; font { pointSize: Theme.pt(20); weight: Font.Medium } }
                        }
                        Label {
                            text: Format.name(win.dialog === "details" ? win.selected?.id : win.target?.id)
                            font.weight: Font.Medium; width: parent.width; wrapMode: Text.Wrap
                        }
                        Note {
                            text: win.dialog === "details" ? (win.selected ? win.selected.unit + "\n" + win.selected.processes.length + " processes · " + Format.size(win.selected.memory) + " own memory\nCPU " + Format.percent(win.selected.cpu) + "\nDisk read " + Format.rate(win.selected.read) + " · write " + Format.rate(win.selected.write) + "\nPer-app network / GPU: Unavailable\n" + (win.selected.reason || "App scope can be ended.") : "This app has closed.")
                                : win.dialog === "force" ? "Immediately terminates every process in the selected app scope. Unsaved work will be lost. Other app scopes are not targeted."
                                : "Sends SIGTERM to the selected app scope. The app may close without saving. If it does not respond, it stays running until you choose Force end."
                        }
                        Note { visible: win.dialog !== "details"; text: "Memory protection prevents automatic closure only. It does not block this explicit action." }
                        Note { visible: Tasks.error !== ""; text: Tasks.error; color: Theme.rose }
                        Note { visible: win.dialog !== "details" && !win.targetValid; text: "This app has closed or changed. Select it again."; color: Theme.rose }
                        Row {
                            spacing: 8
                            Action { id: cancel; text: win.dialog === "details" ? "Done" : "Cancel"; icon: "x"; onClicked: win.dialog = "" }
                            Action {
                                visible: win.dialog !== "details"
                                text: win.dialog === "force" ? "Force end app" : "End app"
                                icon: win.dialog === "force" ? "x" : "square"; kind: "danger"
                                enabled: Tasks.connected && !Tasks.busy && !win.paused && win.targetValid
                                onClicked: win.send("tasks_end",{unit:win.target.unit,token:win.target.token,force:win.dialog === "force"})
                            }
                        }
                    }
                }
            }
            Keys.onEscapePressed: win.dialog = ""
        }
    }
}
