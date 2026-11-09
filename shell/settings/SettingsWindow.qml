// The Settings window (Figma: "Settings - …"): a sidebar of pages, grouped
// System / Desktop / Personal, a search box, and the open page. Every page
// is a module (a folder with module.json and Page.qml, built in under
// settings/modules or installed in ~/.local/share/whiterose/modules); only
// the open page is loaded.
import QtQuick
import Quickshell
import Quickshell.Widgets
import qs

FloatingWindow {
    id: win
    title: "Settings"
    implicitWidth: 1100
    implicitHeight: 720
    color: "transparent"

    // ---- search ----------------------------------------------------------------
    property string query: ""
    function matches(m) {
        const q = win.query.trim().toLowerCase();
        if (q === "") return true;
        return [m.name, m.description ?? "", ...(m.keywords ?? [])].some(s => s.toLowerCase().includes(q));
    }
    readonly property var groups: [
        { id: "system", label: "System" },
        { id: "desktop", label: "Desktop" },
        { id: "personal", label: "Personal" },
    ]

    // ---- layout ----------------------------------------------------------------
    // Rounded and frameless (Figma): the app draws its own window.
    ClippingRectangle {
    anchors.fill: parent
    radius: win.maximized ? 0 : 16
    color: Theme.bg
    Rectangle {
        id: sidebar
        width: 250
        height: parent.height
        color: Theme.sidebar

        // The window's title, in the strip that moves it.
        Text {
            x: 18; y: 13
            text: "Settings"
            color: Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(12); weight: Font.Medium }
        }
        MouseArea {
            width: parent.width; height: 40
            z: -1
            onPressed: win.startSystemMove()
            onDoubleClicked: win.maximized = !win.maximized
        }

        Rectangle {
            id: searchBox
            x: 12; y: 42
            width: parent.width - 24; height: 34; radius: 10
            color: Theme.surface1
            Icon { x: 11; anchors.verticalCenter: parent.verticalCenter; name: "search"; size: 14; color: Theme.muted }
            TextInput {
                id: search
                anchors { left: parent.left; leftMargin: 32; right: slash.left; rightMargin: 6; verticalCenter: parent.verticalCenter }
                color: Theme.text
                selectionColor: Theme.rose
                font { family: Theme.font; pointSize: Theme.pt(12.5) }
                onTextChanged: win.query = text
                Keys.onEscapePressed: text = ""
                Keys.onReturnPressed: {
                    const first = SettingsState.enabled.find(m => win.matches(m));
                    if (first) SettingsState.open(first.id);
                }
                Text {
                    anchors.verticalCenter: parent.verticalCenter
                    visible: search.text === ""
                    text: "Search settings"
                    color: Theme.faint
                    font: search.font
                }
            }
            Rectangle {
                id: slash
                anchors { right: parent.right; rightMargin: 7; verticalCenter: parent.verticalCenter }
                width: 20; height: 20; radius: 5
                color: Theme.surface2
                Text { anchors.centerIn: parent; text: "/"; color: Theme.muted; font { family: Theme.mono; pointSize: Theme.pt(11) } }
            }
        }

        Flickable {
            anchors { top: searchBox.bottom; topMargin: 14; bottom: footer.top; bottomMargin: 8; left: parent.left; right: parent.right }
            contentHeight: navCol.height
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            Column {
                id: navCol
                x: 12; width: parent.width - 24
                spacing: 2
                Repeater {
                    model: win.groups
                    delegate: Column {
                        id: group
                        required property var modelData
                        readonly property var items: SettingsState.enabled.filter(m => m.category === modelData.id && win.matches(m))
                        width: navCol.width
                        visible: items.length > 0
                        spacing: 2
                        Item {
                            width: parent.width; height: 30
                            Text {
                                x: 8; anchors.bottom: parent.bottom; anchors.bottomMargin: 6
                                text: group.modelData.label
                                color: Theme.faint
                                font { family: Theme.font; pointSize: Theme.pt(10.5); weight: Font.Medium; letterSpacing: 0.8; capitalization: Font.AllUppercase }
                            }
                        }
                        Repeater {
                            model: group.items
                            delegate: NavItem { required property var modelData; module: modelData }
                        }
                    }
                }
            }
        }

        Column {
            id: footer
            anchors { bottom: parent.bottom; bottomMargin: 14; left: parent.left; leftMargin: 12; right: parent.right; rightMargin: 12 }
            spacing: 2
            Rectangle { width: parent.width; height: 1; color: Theme.surface1 }
            Item { width: 1; height: 6 }
            Repeater {
                model: SettingsState.enabled.filter(m => m.category === "footer" && win.matches(m))
                delegate: NavItem { required property var modelData; module: modelData }
            }
        }
    }

    component NavItem: Rectangle {
        id: nav
        property var module
        readonly property bool on: SettingsState.page === module.id
        width: parent?.width ?? 0
        height: 34
        radius: 9
        color: on ? Theme.surface2 : navMouse.containsMouse ? Theme.surface1 : "transparent"
        Icon { x: 9; anchors.verticalCenter: parent.verticalCenter; name: nav.module.icon; size: 15; color: nav.on ? Theme.rose : Theme.muted }
        Text {
            x: 36; anchors.verticalCenter: parent.verticalCenter
            text: nav.module.name
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(12.5); weight: nav.on ? Font.Medium : Font.Normal }
        }
        MouseArea { id: navMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: SettingsState.open(nav.module.id) }
    }

    // The open page: loaded on entry, unloaded on leaving (Modules page).
    // A page can set `footer` (a Component) for a bar fixed at the bottom,
    // e.g. Apply / Revert on Displays.
    Flickable {
        id: scroller
        anchors { left: sidebar.right; right: parent.right; top: parent.top; bottom: footerBar.visible ? footerBar.top : parent.bottom }
        contentHeight: pageLoader.height + 64
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        Loader {
            id: pageLoader
            x: 28; y: 28
            width: scroller.width - 56
            source: SettingsState.current ? "file://" + SettingsState.current.dir + "/Page.qml" : ""
            onSourceChanged: scroller.contentY = 0
            onStatusChanged: if (status === Loader.Error) console.warn("settings: page failed to load:", source)
        }
    }
    Rectangle {
        id: footerBar
        visible: (pageLoader.item?.footer ?? null) !== null
        anchors { left: sidebar.right; right: parent.right; bottom: parent.bottom }
        height: visible ? 54 : 0
        color: Theme.bg
        Loader {
            anchors { fill: parent; leftMargin: 28; rightMargin: 28 }
            sourceComponent: pageLoader.item?.footer ?? null
        }
    }

    // Window buttons, top right: minimise, maximise (restore), close.
    // The strip beside them moves the window; double-click maximises.
    MouseArea {
        anchors { left: sidebar.right; right: windowButtons.left; top: parent.top }
        height: 24
        onPressed: win.startSystemMove()
        onDoubleClicked: win.maximized = !win.maximized
    }
    Row {
        id: windowButtons
        anchors { top: parent.top; right: parent.right }
        Repeater {
            model: [
                { icon: "minus", run: () => win.minimized = true },
                { icon: win.maximized ? "restore" : "square", run: () => win.maximized = !win.maximized },
                { icon: "x", close: true, run: () => Qt.quit() },
            ]
            delegate: Rectangle {
                required property var modelData
                width: 46; height: 34
                color: !btnMouse.containsMouse ? "transparent" : modelData.close ? "#C42B1C" : Theme.surface2
                Icon {
                    anchors.centerIn: parent
                    name: modelData.icon
                    size: modelData.icon === "x" ? 14 : 12
                    color: btnMouse.containsMouse && modelData.close ? "white" : Theme.text
                }
                MouseArea { id: btnMouse; anchors.fill: parent; hoverEnabled: true; onClicked: modelData.run() }
            }
        }
    }

    }

    // "/" anywhere jumps to search.
    Shortcut { sequence: "/"; onActivated: search.forceActiveFocus() }
    Shortcut { sequence: "Ctrl+Q"; onActivated: Qt.quit() }
    Shortcut { sequence: "Ctrl+W"; onActivated: Qt.quit() }

}
