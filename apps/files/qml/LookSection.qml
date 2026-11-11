// The Sheet's Info, for a folder: its colour and icon (a colour, a symbol
// on it, or a picture of your own). Folded until asked for; changes show
// straight away; Reset makes it plain again.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import WhiteRose.Kit
import WhiteRose.Files

Column {
    id: root
    // Open (the palette shown), or just the heading.
    property bool expanded: false
    spacing: 10
    property string path
    property string name
    property var look: ({ colour: "", symbol: "", image: "" })
    readonly property var choices: JSON.parse(Fs.lookChoices())
    signal changed()

    function show(path, name) {
        root.path = path;
        root.name = name;
        root.look = Object.assign({ colour: "", symbol: "", image: "" }, JSON.parse(Fs.look(path)) ?? {});
    }
    readonly property bool plain: !(root.look.colour || root.look.symbol || root.look.image)
    function set(changes) {
        root.look = Object.assign({}, root.look, changes);
        Fs.setLook(root.path, JSON.stringify(root.look));
        root.changed();
    }

    FileDialog {
        id: picker
        title: "A picture for “" + root.name + "”"
        nameFilters: ["Pictures (*.png *.jpg *.jpeg *.webp *.svg *.gif)"]
        onAccepted: root.set({ image: decodeURIComponent(String(picker.selectedFile).replace(/^file:\/\//, "")) })
    }

    // ---- the heading: what it looks like now; click to change ----
    Item {
        width: parent.width; height: 30
        FolderIcon { id: now; visible: !root.plain; look: root.look; size: 22; anchors.verticalCenter: parent.verticalCenter }
        Text {
            x: root.plain ? 0 : 32
            anchors.verticalCenter: parent.verticalCenter
            text: "Colour and icon"
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(12) }
        }
        Icon { anchors { right: parent.right; verticalCenter: parent.verticalCenter } name: root.expanded ? "chevUp" : "chevDown"; size: 12; color: Theme.muted }
        MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: root.expanded = !root.expanded }
    }
    // ---- colour ----
    Flow {
        visible: root.expanded
        width: parent.width
        spacing: 6
        Repeater {
            model: [""].concat(root.choices.colours)
            delegate: Rectangle {
                required property string modelData
                readonly property bool on: root.look.colour === modelData
                width: 22; height: 22; radius: 11
                color: modelData === "" ? "transparent" : fi.palette[modelData]
                border { width: on ? 2 : modelData === "" ? 1 : 0; color: on ? Theme.text : Theme.muted }
                Icon { visible: modelData === ""; anchors.centerIn: parent; name: "x"; size: 11; color: Theme.muted }
                Rectangle { visible: parent.on && modelData !== ""; anchors.centerIn: parent; width: 7; height: 7; radius: 3.5; color: Theme.bg }
                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: root.set({ colour: modelData, image: "" }) }
            }
        }
        FolderIcon { id: fi; visible: false }
    }
    // ---- symbol ----
    Flow {
        visible: root.expanded
        width: parent.width
        spacing: 4
        Repeater {
            model: [""].concat(root.choices.symbols)
            delegate: Rectangle {
                required property string modelData
                readonly property bool on: root.look.symbol === modelData
                width: 32; height: 30; radius: 8
                color: on ? Theme.tint(0.25) : symArea.containsMouse ? Theme.surface2 : "transparent"
                border { width: on ? 1 : 0; color: Theme.tint(0.6) }
                Icon { anchors.centerIn: parent; name: modelData === "" ? "minus" : modelData; size: 14; color: modelData === "" ? Theme.faint : Theme.text }
                MouseArea { id: symArea; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: root.set({ symbol: modelData, image: "" }) }
            }
        }
    }
    Row {
        visible: root.expanded
        spacing: 6
        Button { anchors.verticalCenter: undefined; text: root.look.image ? "Change picture…" : "Use a picture…"; onClicked: picker.open() }
        Button { anchors.verticalCenter: undefined; text: "Reset"; kind: "ghost"; enabled: !root.plain; onClicked: root.set({ colour: "", symbol: "", image: "" }) }
    }
}
