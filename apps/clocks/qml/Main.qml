// Approved Clocks page, Figma 195:3257. Doto readouts, native kit controls.
import QtQuick
import QtQuick.Window
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import WhiteRose.Kit
import WhiteRose.Clocks
import "Clocks.js" as Format

AppWindow {
    id: win
    title: "Clocks"
    visible: true
    width: 720; height: 640
    minimumWidth: 420; minimumHeight: 360
    property string page: ["timers", "stopwatch", "alarms", "world"].includes(Clock.view) ? Clock.view : "timers"
    readonly property var data: JSON.parse(Clock.state)
    readonly property var timers: data.timers || []
    readonly property var alarms: data.alarms || []
    readonly property var cities: data.cities || []
    readonly property var sw: data.stopwatch || { running: false, elapsed: 0, started: 0, laps: [] }
    readonly property bool wide: width >= 600
    property real baseBoot: Clock.bootTime()
    property real baseWall: data.now || Date.now()
    property real now: baseWall
    property string dialog: ""
    property string editingAlarm: ""
    property bool editingEnabled: true
    property var repeatDays: []
    property var selectedCity: null
    property string localError: ""
    property bool closeWhenDone: false
    property var draftOptions: ({sound:"chime", volume:100, snooze_duration:300000, ring_duration:0, fade_duration:0})
    property var previousOptions: null
    property bool closeAfterPreview: false
    onClosing: close => {
        if (win.closeAfterPreview) close.accepted = false;
        else if (Clock.previewing) {
            close.accepted = false;
            win.closeAfterPreview = true;
            Clock.stopPreview();
        }
    }
    onDialogChanged: {
        Clock.stopPreview();
        if (dialog) Qt.callLater(() => {
            formScroll.contentY = 0;
            if (win.dialog === "timer") timerMinutes.focusField();
            else if (win.dialog === "alarm") alarmHours.focusField();
            else if (win.dialog === "city") cityQuery.focusField();
        })
    }
    onPageChanged: if (Clock.view !== win.page) Clock.view = win.page

    toolbar: Text {
        anchors.verticalCenter: parent.verticalCenter
        text: "Clocks"
        color: Theme.text
        font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.Medium }
    }
    onDataChanged: { win.baseBoot = Clock.bootTime(); win.baseWall = win.data.now || Date.now(); win.now = win.baseWall }
    onActiveChanged: if (active && Clock.connected) Clock.refresh()
    Connections {
        target: Clock
        function onPreviewStopped() {
            if (win.closeAfterPreview) { win.closeAfterPreview = false; win.close() }
        }
        function onSoundImported(sound) { win.draftOptions = Object.assign({}, win.draftOptions, {sound:sound}) }
        function onViewChanged() { win.page = Clock.view; if (win.visibility === Window.Minimized) win.showNormal(); win.raise(); win.requestActivate() }
        function onBusyChanged() {
            if (!Clock.busy && win.closeWhenDone) {
                win.closeWhenDone = false;
                if (!Clock.error) Qt.callLater(() => win.dialog = "");
            }
        }
    }
    Timer {
        interval: win.page === "stopwatch" && win.sw.running ? 30 : 1000
        running: win.visible && win.active
        repeat: true
        onTriggered: win.now = win.baseWall + Clock.bootTime() - win.baseBoot
    }
    Timer {
        interval: 60000; repeat: true
        running: win.visible && win.active && win.page === "world" && Clock.connected
        onTriggered: Clock.refresh()
    }
    function send(action, args, close) {
        win.localError = "";
        if (!Clock.connected || Clock.busy) return;
        win.closeWhenDone = !!close;
        Clock.command(action, JSON.stringify(args || {}));
    }
    function newTimer() {
        timerHours.text = "00"; timerMinutes.text = "25"; timerSeconds.text = "00"; timerName.text = "";
        win.localError = ""; win.dialog = "timer";
    }
    function newAlarm(alarm) {
        win.editingAlarm = alarm ? alarm.id : "";
        win.editingEnabled = alarm ? alarm.enabled : true;
        alarmHours.text = Format.pad(alarm ? alarm.hour : 7);
        alarmMinutes.text = Format.pad(alarm ? alarm.minute : 30);
        alarmName.text = alarm ? alarm.name : "";
        win.repeatDays = alarm ? (alarm.days || []).slice() : [];
        win.draftOptions = Object.assign({}, alarm ? alarm.options : win.data.defaults || win.draftOptions);
        win.localError = ""; win.dialog = "alarm";
    }
    function submitTimer() {
        const ms = (Number(timerHours.text) * 3600 + Number(timerMinutes.text) * 60 + Number(timerSeconds.text)) * 1000;
        if (!timerHours.text || !timerMinutes.text || !timerSeconds.text || Number(timerHours.text) > 168 || Number(timerMinutes.text) > 59 || Number(timerSeconds.text) > 59 || ms <= 0 || ms > 604800000) {
            win.localError = "Choose a duration greater than zero, up to 7 days"; return;
        }
        win.send("timer_add", { name: timerName.text.trim() || "Timer", duration: ms }, true);
    }
    function submitAlarm() {
        if (!alarmHours.text || !alarmMinutes.text || Number(alarmHours.text) > 23 || Number(alarmMinutes.text) > 59) {
            win.localError = "Choose a valid alarm time"; return;
        }
        win.send("alarm_save", { id: win.editingAlarm, name: alarmName.text.trim() || "Alarm", hour: Number(alarmHours.text), minute: Number(alarmMinutes.text), days: win.repeatDays, enabled: win.editingEnabled, options:win.draftOptions }, true);
    }
    function alarmDefaults() {
        win.draftOptions = Object.assign({}, win.data.defaults || win.draftOptions);
        win.localError = ""; win.dialog = "defaults";
    }
    function cancelDialog() {
        if (win.dialog === "alarmOptions") {
            win.draftOptions = win.previousOptions;
            win.dialog = "alarm";
        } else win.dialog = "";
    }
    function addCity() { cityQuery.text = ""; win.selectedCity = null; win.localError = ""; Clock.search(""); win.dialog = "city" }
    function nextLabel(alarm) {
        const next = Format.alarmNext(alarm);
        if (!next) return "Off";
        if (alarm.snooze && next === alarm.snooze) return "Snoozed · " + Qt.formatDateTime(new Date(next), "hh:mm");
        const date = new Date(next), today = new Date(win.now);
        const tomorrow = new Date(today); tomorrow.setDate(today.getDate() + 1);
        if (date.toDateString() === today.toDateString()) return "Today";
        if (date.toDateString() === tomorrow.toDateString()) return "Tomorrow";
        return Qt.formatDateTime(date, "dddd");
    }

    component Label: Text {
        color: Theme.text
        font { family: Theme.font; pointSize: Theme.pt(13) }
        textFormat: Text.PlainText
        Accessible.role: Accessible.StaticText
        Accessible.name: text
    }
    component Card: Rectangle { radius: 12; color: Theme.surface1 }
    component Empty: Card {
        property string heading
        property string description
        width: content.width; height: 180
        Column {
            anchors.centerIn: parent
            width: parent.width - 40; spacing: 12
            Label { width: parent.width; text: parent.parent.heading; horizontalAlignment: Text.AlignHCenter; font.pointSize: Theme.pt(18) }
            Label { width: parent.width; text: parent.parent.description; color: Theme.muted; horizontalAlignment: Text.AlignHCenter; wrapMode: Text.Wrap }
        }
    }

    Item {
        id: appArea
        anchors.fill: parent
        Flickable {
            id: scroll
            enabled: win.dialog === ""
            anchors.fill: parent
            contentHeight: content.implicitHeight + 40
            boundsBehavior: Flickable.StopAtBounds
            clip: true
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
            Column {
                id: content
                x: 20; y: 20
                width: scroll.width - 40
                spacing: 16
                Rectangle {
                    width: Math.min(nav.implicitWidth + 6, parent.width); height: 30
                    radius: 10; color: Theme.surface2
                    Row {
                        id: nav
                        x: 3; y: 1
                        spacing: 2
                        scale: Math.min(1, (content.width - 6) / implicitWidth)
                        transformOrigin: Item.TopLeft
                        Repeater {
                            model: [
                                { id: "timers", label: "Timers", icon: "timer", width: 84 },
                                { id: "stopwatch", label: "Stopwatch", icon: "timer", width: 106 },
                                { id: "alarms", label: "Alarms", icon: "bell", width: 85 },
                                { id: "world", label: "World clock", icon: "globe", width: 111 }
                            ]
                            delegate: Action {
                                required property var modelData
                                text: modelData.label; icon: modelData.icon
                                width: modelData.width; height: 28; radius: 7
                                enabled: true
                                opacity: win.page === modelData.id ? 1 : 0.65
                                color: win.page === modelData.id ? Theme.grip : "transparent"
                                Accessible.role: Accessible.RadioButton
                                Accessible.checkable: true
                                Accessible.checked: win.page === modelData.id
                                onClicked: win.page = modelData.id
                            }
                        }
                    }
                }
                Item {
                    width: parent.width; height: 32
                    Label {
                        anchors.verticalCenter: parent.verticalCenter
                        text: ({timers:"Timers",stopwatch:"Stopwatch",alarms:"Alarms",world:"World clock"})[win.page]
                        font { pointSize: Theme.pt(22); weight: Font.Medium }
                    }
                    Action {
                        anchors.right: parent.right
                        visible: win.page !== "stopwatch"
                        text: win.page === "timers" ? "New timer" : win.page === "alarms" ? "New alarm" : "Add city"
                        icon: "plus"; kind: "primary"; width: 110
                        onClicked: {
                            if (win.page === "timers") win.newTimer();
                            else if (win.page === "alarms") win.newAlarm(null);
                            else win.addCity();
                        }
                    }
                    Action {
                        anchors.right: parent.right; anchors.rightMargin: 118
                        visible: win.page === "alarms"
                        text: "Defaults"; icon: "bell"; width: 104
                        onClicked: win.alarmDefaults()
                    }
                }
                Label {
                    visible: !Clock.connected || Clock.error !== ""
                    width: parent.width
                    text: Clock.error || "Connect to WhiteRose to use Clocks"
                    color: Theme.rose; wrapMode: Text.Wrap
                    Accessible.role: Accessible.StaticText
                    Accessible.name: text
                }
                Repeater {
                    model: win.data.alerts || []
                    delegate: Card {
                        required property var modelData
                        width: content.width; height: 86
                        border { color: Theme.rose; width: 1 }
                        Column {
                            x: 16; y: 12; width: parent.width - 32; spacing: 10
                            Label { width: parent.width; elide: Text.ElideRight; text: modelData.name + (modelData.kind === "timer" ? " finished" : modelData.silenced ? " · silenced" : " · alarm"); font.weight: Font.Medium }
                            Row {
                                spacing: 8
                                Action { text: modelData.kind === "alarm" ? "Snooze " + (modelData.options?.snooze_duration || 300000)/60000 + " min" : "Restart timer"; icon: modelData.kind === "alarm" ? "bell" : "restart"; onClicked: win.send(modelData.kind === "alarm" ? "alert_snooze" : "alert_restart", {id:modelData.id}) }
                                Action { text: "Dismiss"; icon: "check"; kind: "primary"; onClicked: win.send("alert_dismiss", {id:modelData.id}) }
                            }
                        }
                    }
                }
                Column {
                    visible: win.page === "timers"
                    width: parent.width; spacing: 16
                    Empty { visible: win.timers.length === 0; heading: "No timers"; description: "Add a timer to get started." }
                    Repeater {
                        model: win.timers
                        delegate: Card {
                            id: timerCard
                            required property var modelData
                            required property int index
                            readonly property bool large: index === 0 || !win.wide
                            readonly property real timeLeft: Format.remaining(modelData, win.now)
                            width: content.width; height: large ? 270 : 100
                            Accessible.role: Accessible.Grouping
                            Accessible.name: modelData.name + " timer"
                            Column {
                                visible: timerCard.large
                                x: 20; y: 20; width: parent.width - 40; spacing: 12
                                Item {
                                    width: parent.width; height: 24
                                    Label { text: timerCard.modelData.name; width: parent.width - 140; elide: Text.ElideRight; font { pointSize: Theme.pt(16); weight: Font.Medium } }
                                    Label { anchors.right: parent.right; text: Format.duration(timerCard.modelData.duration) + " timer"; color: Theme.muted; font.pointSize: Theme.pt(12) }
                                }
                                Readout { width: parent.width; height: 100; text: Format.duration(timerCard.timeLeft); textSize: 80; Accessible.name: timerCard.modelData.name + " remaining" }
                                Rectangle {
                                    width: parent.width; height: 4; radius: 2; color: Theme.surface2
                                    Rectangle { width: parent.width * Math.min(1, timerCard.timeLeft / timerCard.modelData.duration); height: 4; radius: 2; color: Theme.rose }
                                }
                                Item {
                                    width: parent.width; height: 18
                                    Label { text: timerCard.modelData.running ? "Finishes at " + Qt.formatDateTime(new Date(timerCard.modelData.deadline), "hh:mm") : timerCard.timeLeft ? "Paused" : "Finished"; color: Theme.muted; font.pointSize: Theme.pt(12) }
                                    Label { anchors.right: parent.right; text: timerCard.modelData.running ? "Running" : timerCard.timeLeft === timerCard.modelData.duration ? "Ready" : timerCard.timeLeft ? "Paused" : "Finished"; color: Theme.rose; font.pointSize: Theme.pt(12) }
                                }
                                Row {
                                    spacing: 8; height: 32
                                    Action {
                                        text: timerCard.modelData.running ? "Pause" : timerCard.timeLeft ? "Resume" : "Restart"
                                        icon: timerCard.modelData.running ? "pause" : "play"
                                        kind: "primary"; width: 100; height: 32
                                        onClicked: win.send(timerCard.modelData.running ? "timer_pause" : timerCard.timeLeft ? "timer_resume" : "timer_restart", {id:timerCard.modelData.id})
                                    }
                                    Action { text: "Reset"; icon: "restart"; width: 90; height: 32; onClicked: win.send("timer_reset", {id:timerCard.modelData.id}) }
                                    Action { text: "Delete"; icon: "trash"; kind: "ghost"; height: 32; onClicked: win.send("timer_delete", {id:timerCard.modelData.id}) }
                                }
                            }
                            Item {
                                visible: !timerCard.large
                                anchors { fill: parent; margins: 16 }
                                Column {
                                    anchors.verticalCenter: parent.verticalCenter
                                    width: 130; spacing: 8
                                    Label { width: parent.width; text: timerCard.modelData.name; elide: Text.ElideRight; font.weight: Font.Medium }
                                    Label { text: Format.duration(timerCard.modelData.duration) + " timer"; color: Theme.muted; font.pointSize: Theme.pt(12) }
                                }
                                Readout { x: 170; width: 190; height: parent.height; text: Format.duration(timerCard.timeLeft); textSize: 40; Accessible.name: timerCard.modelData.name + " remaining" }
                                Row {
                                    anchors { right: parent.right; verticalCenter: parent.verticalCenter }
                                    spacing: 8
                                    Action { text: timerCard.modelData.running ? "Pause" : timerCard.timeLeft ? "Resume" : "Restart"; icon: timerCard.modelData.running ? "pause" : "play"; onClicked: win.send(timerCard.modelData.running ? "timer_pause" : timerCard.timeLeft ? "timer_resume" : "timer_restart", {id:timerCard.modelData.id}) }
                                    Action { text: "Delete"; icon: "trash"; kind: "ghost"; onClicked: win.send("timer_delete", {id:timerCard.modelData.id}) }
                                }
                            }
                        }
                    }
                }
                Column {
                    visible: win.page === "stopwatch"
                    width: parent.width; spacing: 16
                    Item {
                        width: parent.width; height: 190
                        Readout { width: parent.width; height: 88; text: Format.duration(Format.elapsed(win.sw, win.now), true); textSize: 64; horizontalAlignment: Text.AlignHCenter; Accessible.name: "Stopwatch elapsed" }
                        Row {
                            y: 104; anchors.horizontalCenter: parent.horizontalCenter; spacing: 10
                            Action { text: win.sw.running ? "Pause" : "Start"; icon: win.sw.running ? "pause" : "play"; kind: "primary"; width: 112; height: 38; onClicked: win.send(win.sw.running ? "stopwatch_pause" : "stopwatch_start") }
                            Action { text: win.sw.running ? "Lap" : "Reset"; icon: win.sw.running ? "plus" : "restart"; width: 112; height: 38; onClicked: win.send(win.sw.running ? "stopwatch_lap" : "stopwatch_reset") }
                        }
                    }
                    Card {
                        width: parent.width
                        height: Math.max(132, lapColumn.implicitHeight + 32)
                        Column {
                            id: lapColumn
                            x: 16; y: 16; width: parent.width - 32
                            Row {
                                width: parent.width; height: 32
                                Label { width: parent.width * 0.15; text: "Lap"; color: Theme.muted; font.pointSize: Theme.pt(11) }
                                Label { width: parent.width * 0.42; text: "Split"; horizontalAlignment: Text.AlignRight; color: Theme.muted; font.pointSize: Theme.pt(11) }
                                Label { width: parent.width * 0.43; text: "Total"; horizontalAlignment: Text.AlignRight; color: Theme.muted; font.pointSize: Theme.pt(11) }
                            }
                            Label { visible: !win.sw.laps || win.sw.laps.length === 0; text: "No laps recorded"; color: Theme.muted }
                            Repeater {
                                model: (win.sw.laps || []).slice().reverse()
                                delegate: Row {
                                    required property real modelData
                                    required property int index
                                    readonly property int number: win.sw.laps.length - index
                                    width: lapColumn.width; height: 44
                                    Accessible.role: Accessible.Grouping
                                    Accessible.name: "Lap " + number
                                    Label { width: parent.width * 0.15; text: Format.pad(parent.number) }
                                    Label { width: parent.width * 0.42; text: Format.duration(modelData - (parent.number > 1 ? win.sw.laps[parent.number - 2] : 0), true); horizontalAlignment: Text.AlignRight; font.family: Theme.mono }
                                    Label { width: parent.width * 0.43; text: Format.duration(modelData, true); horizontalAlignment: Text.AlignRight; color: Theme.muted; font.family: Theme.mono }
                                }
                            }
                        }
                    }
                }
                Column {
                    visible: win.page === "alarms"
                    width: parent.width; spacing: 16
                    Empty { visible: win.alarms.length === 0; heading: "No alarms"; description: "Add an alarm and choose when it repeats." }
                    Repeater {
                        model: win.alarms
                        delegate: Card {
                            id: alarmCard
                            required property var modelData
                            width: content.width; height: win.wide ? 126 : 160
                            Accessible.role: Accessible.Grouping
                            Accessible.name: modelData.name + " alarm"
                            Readout { x: 16; y: 24; width: win.wide ? 182 : 140; height: 60; text: Format.alarmTime(alarmCard.modelData); textSize: 44; color: alarmCard.modelData.enabled ? Theme.text : Theme.muted; Accessible.name: alarmCard.modelData.name + " time" }
                            Column {
                                x: win.wide ? 214 : 172; y: 26
                                width: parent.width - x - (win.wide ? 160 : 16); spacing: 5
                                Label { width: parent.width; text: alarmCard.modelData.name; elide: Text.ElideRight; font { pointSize: Theme.pt(15); weight: Font.Medium } }
                                Label { width: parent.width; text: Format.repeat(alarmCard.modelData.days); elide: Text.ElideRight; color: Theme.muted; font.pointSize: Theme.pt(12) }
                                Label { text: win.nextLabel(alarmCard.modelData); color: alarmCard.modelData.enabled || alarmCard.modelData.snooze ? Theme.rose : Theme.faint; font.pointSize: Theme.pt(11) }
                            }
                            Row {
                                anchors.right: parent.right; anchors.rightMargin: 24
                                y: win.wide ? 46 : 112
                                spacing: 16; height: 30
                                Toggle { label: alarmCard.modelData.name + " enabled"; checked: alarmCard.modelData.enabled; enabled: Clock.connected && !Clock.busy; onToggled: on => win.send("alarm_enable", {id:alarmCard.modelData.id, enabled:on}) }
                                Action { icon: "pencil"; text: ""; width: 28; kind: "ghost"; Accessible.name: "Edit " + alarmCard.modelData.name; onClicked: win.newAlarm(alarmCard.modelData) }
                                Action { icon: "trash"; text: ""; width: 28; kind: "ghost"; Accessible.name: "Delete " + alarmCard.modelData.name; onClicked: win.send("alarm_delete", {id:alarmCard.modelData.id}) }
                            }
                        }
                    }
                }
                Grid {
                    visible: win.page === "world"
                    width: parent.width
                    columns: win.wide ? 2 : 1
                    spacing: 12
                    Repeater {
                        model: win.cities
                        delegate: Card {
                            id: cityCard
                            required property var modelData
                            width: (content.width - (parent.columns - 1) * 12) / parent.columns
                            height: 195
                            Accessible.role: Accessible.Grouping
                            Accessible.name: modelData.name + " clock"
                            Column {
                                x: 16; y: 16; width: parent.width - 32; spacing: 12
                                Item {
                                    width: parent.width; height: 24
                                    Label { width: parent.width - 32; text: cityCard.modelData.name; elide: Text.ElideRight; font { pointSize: Theme.pt(16); weight: Font.Medium } }
                                    Action { anchors.right: parent.right; visible: !cityCard.modelData.local; width: 24; height: 24; icon: "x"; kind: "ghost"; Accessible.name: "Remove " + cityCard.modelData.name; onClicked: win.send("city_delete", {zone:cityCard.modelData.zone}) }
                                }
                                Readout { width: parent.width; height: 64; text: Format.zoneTime(win.now, cityCard.modelData.offset); textSize: 48; Accessible.name: cityCard.modelData.name + " current time" }
                                Column {
                                    spacing: 4
                                    Label { text: Format.zoneDate(win.now, cityCard.modelData.offset); color: Theme.muted; font.pointSize: Theme.pt(12) }
                                    Label { text: cityCard.modelData.local ? "Local time" : cityCard.modelData.offset === win.cities[0].offset ? "Same as local time" : Format.offset(cityCard.modelData.offset - win.cities[0].offset); color: cityCard.modelData.local ? Theme.rose : Theme.muted; font.pointSize: Theme.pt(12) }
                                }
                            }
                        }
                    }
                }
            }
        }
        // A modal within the app content, leaving the native titlebar usable.
        Rectangle {
            anchors.fill: parent
            visible: win.dialog !== ""
            color: "#A3000000"
            MouseArea { anchors.fill: parent }
            Rectangle {
                id: modal
                anchors.centerIn: parent
                width: Math.min(win.dialog === "city" ? 470 : 438, parent.width - 24)
                height: Math.min(form.implicitHeight + modalFooter.implicitHeight + 66, parent.height - 24)
                radius: 16; color: Theme.surface1
                border { color: Theme.surface2; width: 1 }
                Flickable {
                    id: formScroll
                    anchors { fill: parent; margins: 24; bottomMargin: modalFooter.implicitHeight + 42 }
                    contentHeight: form.implicitHeight
                    clip: true
                    boundsBehavior: Flickable.StopAtBounds
                    Column {
                        id: form
                        width: parent.width; spacing: 18
                        Label { text: win.dialog === "timer" ? "New timer" : win.dialog === "alarm" ? win.editingAlarm ? "Edit alarm" : "New alarm" : win.dialog === "defaults" ? "Alarm defaults" : win.dialog === "alarmOptions" ? "Sound & behavior" : "Add city"; font { pointSize: Theme.pt(20); weight: Font.Medium } }
                        Row {
                            visible: win.dialog === "timer"
                            width: parent.width; spacing: 12
                            Column { width: (form.width - 24) / 3; spacing: 8; Label { text: "Hours"; color: Theme.muted; font.pointSize: Theme.pt(12) } TimeField { id: timerHours; label: "Timer hours"; width: parent.width; maximum: 168 } }
                            Column { width: (form.width - 24) / 3; spacing: 8; Label { text: "Minutes"; color: Theme.muted; font.pointSize: Theme.pt(12) } TimeField { id: timerMinutes; label: "Timer minutes"; width: parent.width } }
                            Column { width: (form.width - 24) / 3; spacing: 8; Label { text: "Seconds"; color: Theme.muted; font.pointSize: Theme.pt(12) } TimeField { id: timerSeconds; label: "Timer seconds"; width: parent.width } }
                        }
                        Row {
                            visible: win.dialog === "alarm"
                            anchors.horizontalCenter: parent.horizontalCenter
                            spacing: 12
                            TimeField { id: alarmHours; label: "Alarm hour"; maximum: 23 }
                            Readout { width: 20; height: 72; text: ":"; textSize: 48; color: Theme.muted }
                            TimeField { id: alarmMinutes; label: "Alarm minute" }
                        }
                        Column {
                            visible: win.dialog === "timer"; width: parent.width; spacing: 6
                            Label { text: "Name"; color: Theme.muted; font.pointSize: Theme.pt(12) }
                            Field { id: timerName; width: parent.width; label: "Timer name"; placeholder: "Timer" }
                        }
                        Column {
                            visible: win.dialog === "alarm"; width: parent.width; spacing: 6
                            Label { text: "Name"; color: Theme.muted; font.pointSize: Theme.pt(12) }
                            Field { id: alarmName; width: parent.width; label: "Alarm name"; placeholder: "Alarm" }
                        }
                        Column {
                            visible: win.dialog === "alarm"; width: parent.width; spacing: 8
                            Label { text: "Repeat"; color: Theme.muted; font.pointSize: Theme.pt(12) }
                            Row {
                                width: parent.width; spacing: 6
                                Repeater {
                                    model: ["Mo","Tu","We","Th","Fr","Sa","Su"]
                                    delegate: Action {
                                        required property string modelData
                                        required property int index
                                        text: modelData
                                        width: (form.width - 36) / 7; height: 28
                                        color: win.repeatDays.includes(index) ? Theme.grip : "transparent"
                                        Accessible.role: Accessible.CheckBox
                                        Accessible.checkable: true
                                        Accessible.checked: win.repeatDays.includes(index)
                                        onClicked: win.repeatDays = win.repeatDays.includes(index) ? win.repeatDays.filter(d => d !== index) : win.repeatDays.concat([index])
                                    }
                                }
                            }
                            Label { text: win.repeatDays.length ? Format.repeat(win.repeatDays.slice().sort()) : "Once, at the next occurrence"; color: Theme.muted; font.pointSize: Theme.pt(12) }
                        }
                        Column {
                            width: parent.width; spacing: 8
                            visible: win.dialog === "alarm"
                            Action {
                                text: "Sound & behavior"; icon: "music"; width: parent.width
                                onClicked: { win.previousOptions = Object.assign({}, win.draftOptions); win.localError = ""; win.dialog = "alarmOptions" }
                            }
                            Label {
                                width: parent.width; color: Theme.muted; wrapMode: Text.Wrap; font.pointSize: Theme.pt(11)
                                text: (win.data.sounds || []).find(s => s.id === win.draftOptions.sound)?.name + " · " + win.draftOptions.volume + "% · " + win.draftOptions.snooze_duration/60000 + " min snooze"
                            }
                        }
                        Label {
                            visible: win.dialog === "defaults"; width: parent.width
                            text: "Used for new alarms only. Existing alarms keep their own settings."
                            color: Theme.muted; wrapMode: Text.Wrap; font.pointSize: Theme.pt(12)
                        }
                        AlarmOptions {
                            visible: win.dialog === "defaults" || win.dialog === "alarmOptions"
                            width: parent.width
                            value: win.draftOptions
                            sounds: win.data.sounds || []
                            onEdited: options => win.draftOptions = options
                            onImportRequested: audioPicker.open()
                        }
                        Column {
                            visible: win.dialog === "city"; width: parent.width; spacing: 12
                            Label { text: "Search cities"; color: Theme.muted; font.pointSize: Theme.pt(12) }
                            Item {
                                width: parent.width; height: 36
                                Field { id: cityQuery; width: parent.width; label: "Search cities"; icon: "search"; placeholder: "City or IANA timezone"; onTextChanged: { win.selectedCity = null; searchDelay.restart() } }
                                Timer { id: searchDelay; interval: 350; onTriggered: Clock.search(cityQuery.text) }
                            }
                            Label { visible: Clock.searching; text: "Searching cities"; color: Theme.muted }
                            Label { visible: !Clock.searching && cityQuery.text.length >= 2 && JSON.parse(Clock.results).length === 0 && !Clock.error; text: "No cities found"; color: Theme.muted }
                            Repeater {
                                model: JSON.parse(Clock.results)
                                delegate: Card {
                                    required property var modelData
                                    activeFocusOnTab: true
                                    Keys.onReturnPressed: win.selectedCity = modelData
                                    Keys.onSpacePressed: win.selectedCity = modelData
                                    width: form.width; height: 72
                                    color: win.selectedCity?.zone === modelData.zone ? Theme.grip : Theme.surface2
                                    Accessible.role: Accessible.RadioButton
                                    Accessible.name: modelData.name + ", " + modelData.zone
                                    Accessible.checkable: true
                                    Accessible.checked: win.selectedCity?.zone === modelData.zone
                                    Accessible.onPressAction: win.selectedCity = modelData
                                    Column {
                                        x: 12; y: 12; width: parent.width - 24; spacing: 8
                                        Label { width: parent.width; text: modelData.name + (modelData.country ? ", " + modelData.country : ""); elide: Text.ElideRight; font.weight: Font.Medium }
                                        Label { text: modelData.zone; color: Theme.muted; font.pointSize: Theme.pt(12) }
                                    }
                                    MouseArea { anchors.fill: parent; onClicked: win.selectedCity = modelData }
                                }
                            }
                        }
                    }
                    ScrollBar.vertical: ScrollBar {}
                }
                Column {
                    id: modalFooter
                    anchors { left: parent.left; right: parent.right; bottom: parent.bottom; margins: 24 }
                    spacing: 12
                    Label { visible: win.localError !== "" || Clock.error !== ""; width: parent.width; text: win.localError || Clock.error; wrapMode: Text.Wrap; color: Theme.rose }
                    Item {
                        width: parent.width; height: 36
                        Action { text: "Cancel"; icon: "x"; height: 36; enabled: !Clock.busy; onClicked: win.cancelDialog() }
                        Action {
                            anchors.right: parent.right
                            text: win.dialog === "timer" ? "Start timer" : win.dialog === "alarm" ? "Save alarm" : win.dialog === "defaults" ? "Save defaults" : win.dialog === "alarmOptions" ? "Done" : "Add city"
                            Accessible.name: win.dialog === "city" ? "Add selected city" : text
                            icon: win.dialog === "timer" ? "play" : win.dialog === "city" ? "plus" : "check"
                            kind: "primary"; height: 36
                            enabled: Clock.connected && !Clock.busy && (win.dialog !== "city" || win.selectedCity !== null)
                            onClicked: {
                                if (win.dialog === "timer") win.submitTimer();
                                else if (win.dialog === "alarm") win.submitAlarm();
                                else if (win.dialog === "defaults") win.send("defaults_save", {options:win.draftOptions}, true);
                                else if (win.dialog === "alarmOptions") win.dialog = "alarm";
                                else win.send("city_add", {name:win.selectedCity.name, country:win.selectedCity.country, zone:win.selectedCity.zone}, true);
                            }
                        }
                    }
                }
            }
        }
    }
    FileDialog {
        id: audioPicker
        title: "Choose alarm audio"
        nameFilters: ["Audio files (*.wav *.flac *.ogg *.mp3 *.m4a *.opus)", "All files (*)"]
        onAccepted: Clock.importSound(selectedFile.toString())
    }
    Shortcut { sequence: "Escape"; enabled: win.dialog !== ""; onActivated: Qt.callLater(win.cancelDialog) }
    Shortcut { sequence: "Alt+1"; enabled: win.dialog === ""; onActivated: win.page = "timers" }
    Shortcut { sequence: "Alt+2"; enabled: win.dialog === ""; onActivated: win.page = "stopwatch" }
    Shortcut { sequence: "Alt+3"; enabled: win.dialog === ""; onActivated: win.page = "alarms" }
    Shortcut { sequence: "Alt+4"; enabled: win.dialog === ""; onActivated: win.page = "world" }
}
