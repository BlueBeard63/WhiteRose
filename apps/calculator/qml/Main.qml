// Figma: Calculator — Standard / Scientific / Scientific + History;
// Doto display refinement approved from frame 191:3284.
import QtQuick
import QtQuick.Window
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Calculator
import "Calc.js" as Calc
import "Input.js" as Input

AppWindow {
    id: win
    title: "Calculator"
    visible: true
    width: 420; height: 616
    minimumWidth: 420; minimumHeight: 360

    property bool ready: false
    property bool scientific: false
    property string angle: "deg"
    property bool showHistory: false
    property var history: []
    property bool completed: false
    property string answer: "0"
    property string error: ""
    property string storageError: ""
    property bool copied: false
    readonly property bool historyInline: showHistory && width >= 700
    readonly property var calculation: Calc.calculate(expression.text || "0", { angle: win.angle })

    toolbar: [
        Text {
            anchors.verticalCenter: parent.verticalCenter
            text: "Calculator"
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.Medium }
        }
    ]

    function receive(json) {
        const reply = JSON.parse(json);
        if (!reply.ok) { win.storageError = "Couldn't save history: " + reply.error; return null }
        win.storageError = "";
        win.history = reply.state.history;
        return reply.state;
    }
    function preferences() {
        if (win.ready) win.receive(Store.preferences(win.scientific, win.angle, win.showHistory));
    }
    function sizeForMode() {
        if (!win.maximized) {
            win.width = (win.scientific ? 572 : 420) + (win.showHistory ? 280 : 0);
            win.height = win.scientific ? 658 : 616;
        }
    }
    function setMode(scientific) {
        win.scientific = scientific;
        win.sizeForMode();
        win.preferences();
        expression.forceActiveFocus();
    }
    function toggleHistory() {
        win.showHistory = !win.showHistory;
        win.sizeForMode();
        win.preferences();
    }
    function setAngle(angle) {
        win.angle = angle;
        win.completed = false;
        win.error = "";
        win.preferences();
    }
    function clear() {
        expression.text = "";
        win.error = "";
        win.completed = false;
        expression.forceActiveFocus();
    }
    function equals() {
        if (!win.calculation.ok) { win.error = win.calculation.error; return }
        if (!expression.text.trim()) return;
        const result = win.calculation;
        win.receive(Store.record(expression.text.trim(), result.result, win.angle));
        win.answer = String(result.value);
        win.completed = true;
        win.error = "";
        expression.forceActiveFocus();
    }
    function insert(value) {
        if (win.completed) {
            expression.text = /^[+−*/÷×^%]$/.test(value) ? win.answer : "";
            expression.cursorPosition = expression.length;
        }
        if (expression.selectionEnd > expression.selectionStart)
            expression.remove(expression.selectionStart, expression.selectionEnd);
        expression.insert(expression.cursorPosition, value);
        win.completed = false;
        win.error = "";
        expression.forceActiveFocus();
    }
    function transform(operation) {
        const edit = Input.transform(expression.text, expression.cursorPosition,
            expression.selectionStart, expression.selectionEnd, operation);
        expression.text = edit.text;
        expression.cursorPosition = edit.cursor;
        win.completed = false;
        win.error = "";
        expression.forceActiveFocus();
    }
    function key(value) {
        if (value === "=") { win.equals(); return }
        if (value === "C") { win.clear(); return }
        if (value === "CE") { win.transform("clear"); return }
        if (value === "⌫") {
            if (expression.selectionEnd > expression.selectionStart) expression.remove(expression.selectionStart, expression.selectionEnd);
            else if (expression.cursorPosition > 0) expression.remove(expression.cursorPosition - 1, expression.cursorPosition);
            win.completed = false; win.error = ""; expression.forceActiveFocus(); return;
        }
        const unary = { "%": "percent", "1/x": "reciprocal", "x²": "square", "√x": "sqrt", "±": "negate", "n!": "factorial" };
        if (Object.prototype.hasOwnProperty.call(unary, value)) { win.transform(unary[value]); return }
        if (value === "xʸ") { win.insert("^"); return }
        if (/^(sin|cos|tan|asin|acos|atan|ln|log|exp|abs)$/.test(value)) {
            if (expression.selectionEnd > expression.selectionStart || win.completed) win.transform(value);
            else win.insert(value + "(");
            return;
        }
        win.insert(value);
    }
    function copyResult(value) {
        Store.copy(value);
        win.copied = true;
        copiedTimer.restart();
    }
    function reuse(entry) {
        win.angle = entry.angle;
        expression.text = entry.expression;
        expression.cursorPosition = expression.length;
        win.error = "";
        win.completed = false;
        if (!win.historyInline) { win.showHistory = false; win.sizeForMode() }
        win.preferences();
        expression.forceActiveFocus();
    }
    Component.onCompleted: {
        const state = win.receive(Store.load());
        if (state) {
            win.scientific = state.scientific;
            win.angle = state.angle;
            win.showHistory = state.show_history;
        }
        win.sizeForMode();
        win.ready = true;
        expression.forceActiveFocus();
    }
    // Another Calculator window may have added/cleared history.
    onActiveChanged: if (active && ready) win.receive(Store.load())
    Timer { id: copiedTimer; interval: 1200; onTriggered: win.copied = false }
    // App-local: Doto is an instrument display, not a desktop-wide font.
    FontLoader { id: displayFont; source: Qt.resolvedUrl("../fonts/Doto.ttf") }

    Item {
        id: calcArea
        anchors { left: parent.left; top: parent.top; bottom: parent.bottom }
        width: parent.width - (win.historyInline ? 280 : 0)
        Flickable {
            id: scroll
            anchors.fill: parent
            contentHeight: Math.max(height, body.implicitHeight + 28)
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar { policy: ScrollBar.AsNeeded }
            Column {
                id: body
                x: 16; y: 16
                width: scroll.width - 32
                spacing: 12
                Item {
                    id: modes
                    width: parent.width; height: 30
                    Segmented {
                        value: win.scientific
                        options: [{ label: "Standard", value: false }, { label: "Scientific", value: true }]
                        onPicked: value => win.setMode(value)
                    }
                    Button {
                        anchors.right: parent.right
                        text: "History"
                        kind: win.showHistory ? "primary" : "plain"
                        onClicked: win.toggleHistory()
                    }
                }
                Rectangle {
                    id: display
                    width: parent.width; height: 144; radius: 12
                    color: Theme.surface1
                    TextInput {
                        id: expression
                        x: 12; y: 12
                        width: parent.width - 24; height: 28
                        horizontalAlignment: TextInput.AlignRight
                        verticalAlignment: TextInput.AlignVCenter
                        color: Theme.muted
                        selectionColor: Theme.rose
                        selectedTextColor: Theme.onAccent
                        font { family: displayFont.name; pointSize: Theme.pt(16); weight: Font.Normal }
                        maximumLength: 4096
                        selectByMouse: true
                        clip: true
                        focus: true
                        Accessible.role: Accessible.EditableText
                        Accessible.name: "Expression"
                        onTextEdited: { win.completed = false; win.error = ""; win.copied = false }
                        Keys.onReturnPressed: win.equals()
                        Keys.onEnterPressed: win.equals()
                        Keys.onEscapePressed: win.clear()
                        Keys.onPressed: event => {
                            if (event.text === "=") { event.accepted = true; win.equals() }
                            else if (win.completed && event.text && event.text.charCodeAt(0) >= 32 && event.text.charCodeAt(0) !== 127
                                && !(event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))) {
                                event.accepted = true;
                                win.insert(event.text);
                            }
                        }
                        Text {
                            anchors.fill: parent
                            visible: expression.text === ""
                            text: "0"
                            horizontalAlignment: Text.AlignRight
                            verticalAlignment: Text.AlignVCenter
                            color: Theme.faint
                            font: expression.font
                        }
                    }
                    Text {
                        id: result
                        x: 12; y: 40
                        width: parent.width - 24; height: 60
                        text: win.error ? "Error" : win.calculation.ok ? win.calculation.result : "…"
                        textFormat: Text.PlainText
                        horizontalAlignment: Text.AlignRight
                        verticalAlignment: Text.AlignVCenter
                        font { family: displayFont.name; pointSize: Theme.pt(52); weight: Font.Bold }
                        fontSizeMode: Text.HorizontalFit
                        minimumPointSize: Theme.pt(16)
                        color: win.error ? Theme.rose : Theme.text
                        Accessible.role: Accessible.StaticText
                        Accessible.name: "Result"
                        Accessible.description: text
                    }
                    Text {
                        x: 12; y: 108
                        width: parent.width - 86; height: 22
                        text: win.error || (win.calculation.ok
                            ? (win.scientific ? win.angle.toUpperCase() + " · live result" : "Live result")
                            : "Finish the expression")
                        verticalAlignment: Text.AlignVCenter
                        elide: Text.ElideRight
                        color: win.error ? Theme.rose : Theme.muted
                        font { family: Theme.font; pointSize: Theme.pt(11) }
                        Accessible.role: Accessible.StaticText
                        Accessible.name: text
                    }
                    Button {
                        anchors.verticalCenter: undefined
                        x: parent.width - width - 12; y: 108
                        height: 22
                        text: win.copied ? "Copied" : "Copy"
                        enabled: win.calculation.ok && !win.error
                        onClicked: win.copyResult(win.calculation.result)
                    }
                }
                Item {
                    width: parent.width; height: 30
                    visible: win.scientific
                    Segmented {
                        value: win.angle
                        options: [{ label: "DEG", value: "deg" }, { label: "RAD", value: "rad" }]
                        onPicked: value => win.setAngle(value)
                    }
                    Row {
                        anchors.right: parent.right
                        height: 30; spacing: 6
                        Repeater {
                            model: ["π", "e", "exp", "abs"]
                            delegate: Button {
                                required property string modelData
                                width: 52; height: 28
                                text: modelData
                                onClicked: win.key(modelData)
                            }
                        }
                    }
                }
                Grid {
                    id: keypad
                    width: parent.width
                    columns: win.scientific ? 6 : 4
                    spacing: 6
                    Repeater {
                        model: Input.keys(win.scientific)
                        delegate: CalcKey {
                            required property string modelData
                            label: modelData
                            width: (keypad.width - (keypad.columns - 1) * keypad.spacing) / keypad.columns
                            height: 48
                            onClicked: win.key(modelData)
                        }
                    }
                }
                Text {
                    width: parent.width
                    visible: win.storageError !== ""
                    text: win.storageError
                    wrapMode: Text.Wrap
                    color: Theme.rose
                    font { family: Theme.font; pointSize: Theme.pt(11) }
                    Accessible.role: Accessible.StaticText
                    Accessible.name: text
                }
            }
        }
    }
    Rectangle {
        id: historyPanel
        visible: win.showHistory
        anchors { right: parent.right; top: parent.top; bottom: parent.bottom }
        width: win.historyInline ? 280 : parent.width
        color: Theme.sidebar
        Item {
            id: historyHeading
            x: 20; y: 16; width: parent.width - 40; height: 30
            Text {
                anchors.verticalCenter: parent.verticalCenter
                text: "History"
                color: Theme.text
                font { family: Theme.font; pointSize: Theme.pt(15); weight: Font.Medium }
            }
            Row {
                anchors.right: parent.right
                height: 30; spacing: 8
                Button { visible: !win.historyInline; text: "Back"; onClicked: win.toggleHistory() }
                Button { text: "Clear"; enabled: win.history.length > 0; onClicked: win.receive(Store.clear()) }
            }
        }
        Text {
            id: historyLabel
            x: 20; y: 62
            text: "SAVED ON THIS DEVICE"
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(10); weight: Font.Medium }
        }
        ListView {
            id: historyList
            anchors { left: parent.left; right: parent.right; top: historyLabel.bottom; bottom: parent.bottom; margins: 20; topMargin: 18 }
            clip: true
            spacing: 12
            model: win.history
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            delegate: Rectangle {
                id: entry
                required property var modelData
                width: historyList.width; height: 108; radius: 10
                color: Theme.surface1
                Accessible.role: Accessible.ListItem
                Accessible.name: modelData.expression + " = " + modelData.result
                Accessible.description: modelData.angle.toUpperCase()
                Accessible.onPressAction: win.reuse(entry.modelData)
                Text {
                    x: 12; y: 10; width: parent.width - 24
                    text: entry.modelData.expression
                    textFormat: Text.PlainText
                    elide: Text.ElideRight
                    color: Theme.muted
                    font { family: Theme.mono; pointSize: Theme.pt(12) }
                }
                Text {
                    x: 12; y: 34; width: parent.width - 62; height: 28
                    text: entry.modelData.result
                    elide: Text.ElideRight
                    color: Theme.text
                    font { family: Theme.mono; pointSize: Theme.pt(20) }
                }
                Text {
                    anchors { right: parent.right; rightMargin: 12; top: parent.top; topMargin: 44 }
                    text: entry.modelData.angle.toUpperCase()
                    color: Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(10) }
                }
                Row {
                    x: 12; y: 76
                    height: 22; spacing: 6
                    CalcKey { label: "Reuse expression"; textSize: 11; width: (entry.width - 30) / 2; height: 22; onClicked: win.reuse(entry.modelData) }
                    CalcKey { label: "Copy result"; textSize: 11; width: (entry.width - 30) / 2; height: 22; onClicked: win.copyResult(entry.modelData.result) }
                }
            }
            Text {
                width: parent.width
                visible: win.history.length === 0
                text: "No calculations yet.\nPress Enter or = to save one here."
                wrapMode: Text.Wrap
                color: Theme.muted
                font { family: Theme.font; pointSize: Theme.pt(12) }
                Accessible.role: Accessible.StaticText
                Accessible.name: "No calculations yet"
            }
        }
    }
    Shortcut { sequence: "Ctrl+H"; onActivated: win.toggleHistory() }
    Shortcut { sequence: "Alt+1"; onActivated: win.setMode(false) }
    Shortcut { sequence: "Alt+2"; onActivated: win.setMode(true) }
    Shortcut { sequence: "Escape"; onActivated: win.clear() }
}
