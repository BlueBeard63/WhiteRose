// Colour and icon for a folder: a colour, a symbol on it, or a picture of
// your own. Changes show straight away; Reset makes it plain again.
import QtQuick
import QtQuick.Controls.Basic
import QtQuick.Dialogs
import WhiteRose.Kit
import WhiteRose.Files

Popup {
    id: root
    property string path
    property string name
    property var look: ({ colour: "", symbol: "", image: "" })
    readonly property var choices: JSON.parse(Fs.lookChoices())
    signal changed()

    function show(path, name) {
        root.path = path;
        root.name = name;
        root.look = Object.assign({ colour: "", symbol: "", image: "" }, JSON.parse(Fs.look(path)) ?? {});
        root.open();
    }
    function set(changes) {
        root.look = Object.assign({}, root.look, changes);
        Fs.setLook(root.path, JSON.stringify(root.look));
        root.changed();
    }

    parent: Overlay.overlay
    anchors.centerIn: parent
    width: 400
    modal: true
    focus: true
    padding: 22
    background: Rectangle { color: Theme.surface1; radius: 16; border { width: 1; color: Theme.surface2 } }
    Overlay.modal: Rectangle { color: Qt.rgba(0, 0, 0, 0.35) }

    FileDialog {
        id: picker
        title: "A picture for “" + root.name + "”"
        nameFilters: ["Pictures (*.png *.jpg *.jpeg *.webp *.svg *.gif)"]
        onAccepted: root.set({ image: decodeURIComponent(String(picker.selectedFile).replace(/^file:\/\//, "")) })
    }

    contentItem: Column {
        spacing: 16
        Row {
            spacing: 14
            FolderIcon { look: root.look; size: 56; visible: !!(root.look.colour || root.look.symbol || root.look.image) }
            Image {
                visible: !(root.look.colour || root.look.symbol || root.look.image)
                width: 56; height: 56; sourceSize: Qt.size(112, 112)
                source: "file://" + Fs.typeIcon("inode/directory", true, 128)
            }
            Column {
                anchors.verticalCenter: parent.verticalCenter
                Text {
                    text: root.name
                    width: 270
                    elide: Text.ElideMiddle
                    color: Theme.text
                    font { family: Theme.font; pointSize: Theme.pt(14); weight: Font.DemiBold }
                }
                Text { text: "Colour and icon"; color: Theme.muted; font { family: Theme.font; pointSize: Theme.pt(11.5) } }
            }
        }

        // ---- colour --------------------------------------------------------------------
        Flow {
            width: parent.width
            spacing: 6
            Repeater {
                model: [""].concat(root.choices.colours)
                delegate: Rectangle {
                    required property string modelData
                    readonly property bool on: root.look.colour === modelData
                    width: 25; height: 25; radius: 12.5
                    color: modelData === "" ? "transparent" : fi.palette[modelData]
                    border { width: on ? 2 : modelData === "" ? 1 : 0; color: on ? Theme.text : Theme.muted }
                    Icon { visible: modelData === ""; anchors.centerIn: parent; name: "x"; size: 12; color: Theme.muted }
                    Rectangle {
                        visible: parent.on && modelData !== ""
                        anchors.centerIn: parent; width: 8; height: 8; radius: 4; color: Theme.bg
                    }
                    MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: root.set({ colour: modelData, image: "" }) }
                }
            }
            FolderIcon { id: fi; visible: false }
        }

        // ---- symbol ---------------------------------------------------------------------
        Grid {
            columns: 8
            spacing: 6
            Repeater {
                model: [""].concat(root.choices.symbols)
                delegate: Rectangle {
                    required property string modelData
                    readonly property bool on: root.look.symbol === modelData
                    width: 38; height: 34; radius: 9
                    color: on ? Theme.tint(0.25) : symArea.containsMouse ? Theme.surface2 : "transparent"
                    border { width: on ? 1 : 0; color: Theme.tint(0.6) }
                    Icon { anchors.centerIn: parent; name: modelData === "" ? "minus" : modelData; size: 15; color: modelData === "" ? Theme.faint : Theme.text }
                    MouseArea { id: symArea; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: root.set({ symbol: modelData, image: "" }) }
                }
            }
        }

        Button { text: root.look.image ? "Change picture…" : "Use a picture…"; anchors.verticalCenter: undefined; onClicked: picker.open() }
        Row {
            anchors.right: parent.right
            spacing: 8
            Button {
                text: "Reset"
                kind: "ghost"
                enabled: !!(root.look.colour || root.look.symbol || root.look.image)
                onClicked: root.set({ colour: "", symbol: "", image: "" })
            }
            Button { text: "Done"; kind: "primary"; onClicked: root.close() }
        }
    }
}
