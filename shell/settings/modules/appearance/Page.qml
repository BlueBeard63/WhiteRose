// Appearance (Figma: "Settings - Appearance"): theme, accent colour,
// wallpaper, font and text size. Everything applies live across the shell
// (Theme.qml and Background.qml follow these settings).
import QtQuick
import QtQuick.Dialogs
import Quickshell
import Quickshell.Widgets
import qs
import qs.settings
import "../../../Wallpapers.js" as W

Column {
    id: page
    spacing: 16

    readonly property string theme: Daemon.setting("appearance.theme", "dark")
    readonly property string accent: Daemon.setting("appearance.accent", "#E8A3AE")
    readonly property string home: Quickshell.env("HOME") ?? ""

    PageHeader { title: "Appearance"; subtitle: "Theme, accent and wallpaper. Changes apply live across the shell." }

    // ---- theme ------------------------------------------------------------------
    Row {
        width: parent.width
        spacing: 12
        Repeater {
            model: [
                { id: "dark", name: "Dark", say: "Default", sides: ["dark"] },
                { id: "light", name: "Light", say: "Bright panels", sides: ["light"] },
                { id: "auto", name: "Auto", say: "Follows sunset and sunrise", sides: ["dark", "light"] },
            ]
            delegate: Rectangle {
                id: card
                required property var modelData
                readonly property bool on: page.theme === modelData.id
                width: (page.width - 24) / 3
                height: 176
                radius: 16
                color: Theme.surface1
                border { width: on ? 1.5 : 0; color: Theme.pearl }

                // A small desktop: the wallpaper, the island and a window.
                ClippingRectangle {
                    id: preview
                    x: 10; y: 10
                    width: parent.width - 20; height: 108
                    radius: 10
                    color: "black"
                    WallpaperView {
                        anchors.fill: parent
                        wallpaper: Wallpaper.current
                        sourceSize: Qt.size(400, 220)
                    }
                    Repeater {
                        model: card.modelData.sides
                        delegate: Item {
                            required property string modelData
                            required property int index
                            readonly property bool lite: modelData === "light"
                            readonly property int n: card.modelData.sides.length
                            x: index * preview.width / n; width: preview.width / n; height: preview.height
                            clip: true
                            // The bar.
                            Rectangle { width: parent.width; height: 7; color: parent.lite ? "#FFFFFF" : "#0F0F0F" }
                            // A window.
                            Rectangle {
                                anchors.centerIn: parent
                                anchors.verticalCenterOffset: 6
                                width: Math.min(parent.width - 24, 126); height: 60; radius: 5
                                color: parent.lite ? "#F4F3F1" : "#0F0F0F"
                                Rectangle { width: parent.width * 0.18; height: parent.height; radius: 5; color: parent.parent.lite ? "#ECEBE8" : "#1C1C1F" }
                            }
                        }
                    }
                    // The island (dark in dark, light in light; split for auto).
                    Rectangle {
                        anchors.horizontalCenter: parent.horizontalCenter
                        width: 62; height: 14; radius: 6
                        color: card.modelData.sides[0] === "light" ? "#FFFFFF" : "#0F0F0F"
                    }
                }
                Row {
                    x: 14; y: preview.y + preview.height + 14
                    spacing: 10
                    Rectangle {
                        anchors.verticalCenter: parent.verticalCenter
                        width: 16; height: 16; radius: 8
                        color: "transparent"
                        border { width: 1.5; color: card.on ? Theme.pearl : Theme.muted }
                        Rectangle { anchors.centerIn: parent; visible: card.on; width: 8; height: 8; radius: 4; color: Theme.pearl }
                    }
                    Column {
                        spacing: 1
                        Text { text: card.modelData.name; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.Medium } }
                        Text {
                            text: card.modelData.id === "auto" && Daemon.setting("weather.latitude", null) === null
                                ? "Light 07:00–19:00 (set a weather city for sunrise and sunset)" : card.modelData.say
                            color: Theme.muted
                            width: card.width - 50
                            elide: Text.ElideRight
                            font { family: Theme.font; pointSize: Theme.pt(11) }
                        }
                    }
                }
                MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: Daemon.set("appearance.theme", card.modelData.id) }
            }
        }
    }

    // ---- accent -------------------------------------------------------------------
    Card {
        SettingRow {
            title: "Accent colour"
            subtitle: "Toggles, highlights and the active workspace"
            Repeater {
                // Rose, pearl, lavender, blue, mint, sand, coral (Figma).
                model: ["#E8A3AE", "#F3E6E8", "#B9A3E8", "#8FB8F2", "#9BD5A8", "#F2C98F", "#F2A68F"]
                delegate: Rectangle {
                    required property string modelData
                    readonly property bool on: page.accent.toUpperCase() === modelData.toUpperCase()
                    anchors.verticalCenter: parent.verticalCenter
                    width: 28; height: 28; radius: 14
                    color: "transparent"
                    border { width: on ? 1.5 : 0; color: Theme.text }
                    Rectangle { anchors.centerIn: parent; width: 22; height: 22; radius: 11; color: parent.modelData }
                    MouseArea { anchors.fill: parent; cursorShape: Qt.PointingHandCursor; onClicked: Daemon.set("appearance.accent", parent.modelData) }
                }
            }
        }
    }

    // ---- wallpaper -------------------------------------------------------------------
    Component.onCompleted: Wallpaper.refresh()
    Card {
        Item {
            width: parent.width; height: 62
            Column {
                x: 14; anchors.verticalCenter: parent.verticalCenter
                spacing: 2
                Text { text: "Wallpaper"; color: Theme.text; font { family: Theme.font; pointSize: Theme.pt(13); weight: Font.Medium } }
                Text {
                    text: Wallpaper.rotateMinutes > 0 ? W.rotationText(Wallpaper.rotateMinutes).replace(/^c/, "C") : "Pick one, or add a folder of images"
                    color: Theme.muted
                    font { family: Theme.font; pointSize: Theme.pt(11.5) }
                }
            }
            Row {
                anchors { right: parent.right; rightMargin: 14; verticalCenter: parent.verticalCenter }
                spacing: 8
                Choice {
                    value: Wallpaper.rotateMinutes
                    options: [
                        { label: "Never changes", value: 0 }, { label: "Every 15 min", value: 15 }, { label: "Every 30 min", value: 30 },
                        { label: "Every hour", value: 60 }, { label: "Every day", value: 1440 },
                    ]
                    onPicked: v => Daemon.set("wallpaper.rotateMinutes", v)
                }
                Button { text: "Add folder"; icon: "plus"; onClicked: folderDialog.open() }
            }
        }
        // The folders, removable.
        Flow {
            visible: Wallpaper.folders.length > 0
            x: 14; width: parent.width - 28
            spacing: 6
            bottomPadding: 10
            Repeater {
                model: Wallpaper.folders
                delegate: Rectangle {
                    required property string modelData
                    width: folderRow.implicitWidth + 20; height: 24; radius: 12
                    color: folderMouse.containsMouse ? Theme.grip : Theme.surface2
                    Row {
                        id: folderRow
                        anchors.centerIn: parent
                        spacing: 6
                        Icon { anchors.verticalCenter: parent.verticalCenter; name: "folder"; size: 11; color: Theme.muted }
                        Text { anchors.verticalCenter: parent.verticalCenter; text: W.tildify(W.expand(modelData, page.home), page.home); color: Theme.text; font { family: Theme.mono; pointSize: Theme.pt(10.5) } }
                        Icon { anchors.verticalCenter: parent.verticalCenter; visible: folderMouse.containsMouse; name: "x"; size: 10; color: Theme.muted }
                    }
                    MouseArea {
                        id: folderMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: Daemon.set("wallpaper.folders", Wallpaper.folders.filter(f => f !== modelData))
                    }
                }
            }
        }
        Flow {
            x: 14; width: parent.width - 28
            spacing: 10
            bottomPadding: 14
            Repeater {
                model: Wallpaper.all
                delegate: Item {
                    id: thumb
                    required property string modelData
                    readonly property bool on: Wallpaper.current === modelData
                    readonly property bool added: !W.isBuiltin(modelData) && Wallpaper.files.some(f => W.expand(f, page.home) === modelData)
                    width: 148; height: 78
                    ClippingRectangle {
                        anchors.fill: parent
                        radius: 12
                        color: Theme.surface2
                        WallpaperView {
                            anchors.fill: parent
                            wallpaper: thumb.modelData
                            sourceSize: Qt.size(296, 156)
                        }
                    }
                    Rectangle {
                        anchors.fill: parent
                        radius: 12
                        color: "transparent"
                        border { width: thumb.on ? 2 : 1; color: thumb.on ? Theme.pearl : Theme.surface2 }
                    }
                    // Images you added one by one can be taken off again.
                    Rectangle {
                        visible: thumb.added && thumbMouse.containsMouse
                        anchors { top: parent.top; right: parent.right; margins: 6 }
                        width: 20; height: 20; radius: 10
                        color: removeMouse.containsMouse ? Theme.rose : Qt.rgba(0, 0, 0, 0.6)
                        Icon { anchors.centerIn: parent; name: "x"; size: 10; color: removeMouse.containsMouse ? Theme.onAccent : "white" }
                        MouseArea {
                            id: removeMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: Daemon.set("wallpaper.files", Wallpaper.files.filter(f => W.expand(f, page.home) !== thumb.modelData))
                        }
                    }
                    MouseArea {
                        id: thumbMouse
                        anchors.fill: parent
                        z: -1
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: Wallpaper.set(thumb.modelData)
                    }
                }
            }
            // Add one image.
            Rectangle {
                width: 78; height: 78; radius: 12
                color: addMouse.containsMouse ? Theme.grip : Theme.surface2
                Icon { anchors.centerIn: parent; name: "plus"; size: 16; color: Theme.muted }
                MouseArea { id: addMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: fileDialog.open() }
            }
        }
    }

    FolderDialog {
        id: folderDialog
        title: "Add a wallpaper folder"
        currentFolder: "file://" + page.home + "/Pictures"
        onAccepted: {
            const path = decodeURIComponent(String(selectedFolder).replace(/^file:\/\//, ""));
            const tilded = W.tildify(path, page.home);
            if (!Wallpaper.folders.includes(tilded) && !Wallpaper.folders.includes(path)) Daemon.set("wallpaper.folders", Wallpaper.folders.concat([tilded]));
        }
    }
    FileDialog {
        id: fileDialog
        title: "Choose a wallpaper"
        currentFolder: "file://" + page.home + "/Pictures"
        nameFilters: ["Images (*.png *.jpg *.jpeg *.webp *.avif *.bmp)"]
        onAccepted: {
            const path = decodeURIComponent(String(selectedFile).replace(/^file:\/\//, ""));
            const tilded = W.tildify(path, page.home);
            if (!Wallpaper.files.includes(tilded)) Daemon.set("wallpaper.files", Wallpaper.files.concat([tilded]));
            Wallpaper.set(path);
        }
    }

    // ---- type ----------------------------------------------------------------------
    // Fonts made for interfaces, offered if installed (plus whatever is set).
    readonly property var fontChoices: {
        const installed = Qt.fontFamilies();
        const wanted = ["Geist", "Inter", "Cantarell", "Ubuntu", "Noto Sans", "Fira Sans", "Roboto", "Source Sans 3", "IBM Plex Sans", "Open Sans", "DejaVu Sans", "Liberation Sans"];
        const list = wanted.filter(f => installed.includes(f));
        if (!list.includes(Theme.font)) list.unshift(Theme.font);
        return list;
    }
    Card {
        SettingRow {
            title: "Font"
            subtitle: "Interface text"
            Choice {
                value: Theme.font
                options: page.fontChoices.map(f => ({ label: f, value: f }))
                onPicked: v => Daemon.set("appearance.font", v)
            }
        }
        SettingRow {
            title: "Interface size"
            subtitle: "Text across the shell and Settings"
            Segmented {
                value: Daemon.setting("appearance.size", "default")
                options: [{ label: "Compact", value: "compact" }, { label: "Default", value: "default" }, { label: "Large", value: "large" }]
                onPicked: v => Daemon.set("appearance.size", v)
            }
        }
    }
}
