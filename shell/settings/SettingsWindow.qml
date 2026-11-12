// The Settings window (Figma: "Settings - …"): a sidebar of pages, grouped
// System / Desktop / Personal, a search box, and the open page. Narrower
// than 900px (a tile, Figma: "Settings - narrow · …") the sidebar becomes
// an icon rail, and its menu button slides the full sidebar over the page. Every page
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
        return [m.name, m.description ?? "", ...(m.keywords ?? [])].some(s => s.toLowerCase().includes(q))
            || (m.pages ?? []).some(p => win.subMatches(p));
    }
    function subMatches(p) {
        const q = win.query.trim().toLowerCase();
        return q !== "" && [p.name, ...(p.keywords ?? [])].some(s => s.toLowerCase().includes(q));
    }
    readonly property var groups: [
        { id: "system", label: "System" },
        { id: "desktop", label: "Desktop" },
        { id: "personal", label: "Personal" },
    ]

    // ---- narrow: the rail, and the sidebar over the page --------------------------
    readonly property bool narrow: width < 900
    property bool drawerOpen: false
    onNarrowChanged: drawerOpen = false
    function openDrawer(searching) {
        win.drawerOpen = true;
        if (searching) search.forceActiveFocus();
    }
    Connections {
        target: SettingsState
        // Picking a page closes the sidebar over it.
        function onPageChanged() { win.drawerOpen = false }
        function onSubChanged() { win.drawerOpen = false }
    }
    // The page's padding: less where it's narrow.
    readonly property int pad: scroller.width < 560 ? 16 : 28
    // Where the page starts: right of the sidebar, or of the rail.
    readonly property Item navEdge: narrow ? rail : sidebar

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
        // Narrow: over the page while open, out of sight (and out of the
        // accessibility tree) while not.
        x: !win.narrow || win.drawerOpen ? 0 : -width
        visible: !win.narrow || win.drawerOpen || slide.running
        z: win.narrow ? 20 : 0
        Behavior on x { NumberAnimation { id: slide; duration: Theme.fastAnim; easing.type: Easing.OutCubic } }
        // Its shadow over the page.
        Rectangle {
            visible: win.narrow
            anchors { left: parent.right; top: parent.top; bottom: parent.bottom }
            width: 18
            gradient: Gradient {
                orientation: Gradient.Horizontal
                GradientStop { position: 0; color: Qt.rgba(0, 0, 0, 0.35) }
                GradientStop { position: 1; color: "transparent" }
            }
        }

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
                Accessible.role: Accessible.EditableText
                Accessible.name: "Search settings"
                anchors { left: parent.left; leftMargin: 32; right: slash.left; rightMargin: 6; verticalCenter: parent.verticalCenter }
                color: Theme.text
                selectionColor: Theme.rose
                font { family: Theme.font; pointSize: Theme.pt(12.5) }
                onTextChanged: win.query = text
                Keys.onEscapePressed: text = ""
                Keys.onReturnPressed: {
                    const first = SettingsState.enabled.find(m => win.matches(m));
                    if (!first) return;
                    const sub = (first.pages ?? []).find(p => win.subMatches(p));
                    SettingsState.open(sub ? first.id + "/" + sub.id : first.id);
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
            id: navFlick
            anchors { top: searchBox.bottom; topMargin: 14; bottom: footer.top; bottomMargin: 8; left: parent.left; right: parent.right }
            // Scrolls `item` (in the list) fully into sight.
            function reveal(item) {
                const y = item.mapToItem(navCol, 0, 0).y;
                if (y + item.height > navFlick.contentY + navFlick.height) navFlick.contentY = Math.min(navCol.height - navFlick.height, y + item.height - navFlick.height + 8);
                else if (y < navFlick.contentY) navFlick.contentY = Math.max(0, y - 8);
            }
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
                            delegate: Column {
                                id: navGroup
                                required property var modelData
                                width: parent.width
                                spacing: 2
                                NavItem { module: navGroup.modelData }
                                // Opened: its sub-pages come into sight.
                                readonly property bool open: SettingsState.page === navGroup.modelData.id && (navGroup.modelData.pages ?? []).length > 0
                                onOpenChanged: if (open) Qt.callLater(() => navFlick.reveal(navGroup))
                                onHeightChanged: if (open) Qt.callLater(() => navFlick.reveal(navGroup))
                                // Its sub-pages, while it's open (or they match a search).
                                Repeater {
                                    model: (navGroup.modelData.pages ?? []).filter(p => SettingsState.page === navGroup.modelData.id || win.subMatches(p))
                                    delegate: SubNavItem { required property var modelData; module: navGroup.modelData; pageInfo: modelData }
                                }
                            }
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

    // Narrow: the page dims behind the sidebar; a click there closes it.
    Rectangle {
        anchors.fill: parent
        z: 19
        visible: win.narrow && (win.drawerOpen || slide.running)
        color: Qt.rgba(0, 0, 0, win.drawerOpen ? 0.45 : 0)
        Behavior on color { ColorAnimation { duration: Theme.fastAnim } }
        MouseArea { anchors.fill: parent; onClicked: win.drawerOpen = false }
    }

    // Narrow: the rail (Figma: Settings/Rail): the sidebar's pages as icons,
    // named for screen readers and tests; Menu slides the sidebar over the
    // page, Search does so with the search box focused.
    Rectangle {
        id: rail
        visible: win.narrow
        width: visible ? 64 : 0
        height: parent.height
        color: Theme.sidebar
        MouseArea {
            width: parent.width; height: 40
            onPressed: win.startSystemMove()
            onDoubleClicked: win.maximized = !win.maximized
        }
        Column {
            id: railTop
            x: 12; y: 40
            spacing: 2
            RailButton { icon: "panelLeft"; label: "Menu"; onActivated: win.openDrawer(false) }
            RailButton { icon: "search"; label: "Search settings"; onActivated: win.openDrawer(true) }
        }
        Flickable {
            anchors { top: railTop.bottom; topMargin: 6; bottom: railFooter.top; bottomMargin: 6; left: parent.left; right: parent.right }
            contentHeight: railCol.height
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            Column {
                id: railCol
                x: 12
                spacing: 2
                Repeater {
                    model: win.groups
                    delegate: Column {
                        id: railGroup
                        required property var modelData
                        required property int index
                        readonly property var items: SettingsState.enabled.filter(m => m.category === modelData.id)
                        visible: items.length > 0
                        spacing: 2
                        // Between groups: a short line, where the labels were.
                        Item {
                            visible: railGroup.index > 0
                            width: 40; height: 9
                            Rectangle { x: 8; y: 4; width: 24; height: 1; color: Theme.surface1 }
                        }
                        Repeater {
                            model: railGroup.items
                            delegate: RailButton {
                                required property var modelData
                                icon: modelData.icon
                                label: modelData.name
                                tab: true
                                on: SettingsState.page === modelData.id
                                onActivated: SettingsState.open(modelData.id)
                            }
                        }
                    }
                }
            }
        }
        Column {
            id: railFooter
            x: 12
            anchors { bottom: parent.bottom; bottomMargin: 12 }
            spacing: 2
            Item {
                width: 40; height: 9
                Rectangle { x: 8; y: 4; width: 24; height: 1; color: Theme.surface1 }
            }
            Repeater {
                model: SettingsState.enabled.filter(m => m.category === "footer")
                delegate: RailButton {
                    required property var modelData
                    icon: modelData.icon
                    label: modelData.name
                    tab: true
                    on: SettingsState.page === modelData.id
                    onActivated: SettingsState.open(modelData.id)
                }
            }
        }
    }

    component RailButton: Rectangle {
        id: rb
        property string icon
        property string label
        // A page (a tab), or one of the rail's own buttons.
        property bool tab: false
        property bool on: false
        signal activated()
        Accessible.role: tab ? Accessible.PageTab : Accessible.Button
        Accessible.name: label
        Accessible.selected: on
        Accessible.onPressAction: rb.activated()
        width: 40; height: tab ? 28 : 34
        radius: 9
        color: on ? Theme.surface2 : rbMouse.containsMouse ? Theme.surface1 : "transparent"
        Icon { anchors.centerIn: parent; name: rb.icon; size: 16; color: rb.on ? Theme.rose : Theme.muted }
        MouseArea { id: rbMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: rb.activated() }
    }

    component NavItem: Rectangle {
        id: nav
        property var module
        readonly property bool on: SettingsState.page === module.id && SettingsState.sub === ""
        Accessible.role: Accessible.PageTab
        Accessible.name: module.name
        Accessible.selected: on
        Accessible.onPressAction: SettingsState.open(module.id)
        width: parent?.width ?? 0
        height: 34
        radius: 9
        color: on ? Theme.surface2 : navMouse.containsMouse ? Theme.surface1 : "transparent"
        // (One of its sub-pages open: its icon stays lit.)
        readonly property bool within: SettingsState.page === module.id
        Icon { x: 9; anchors.verticalCenter: parent.verticalCenter; name: nav.module.icon; size: 15; color: nav.within ? Theme.rose : Theme.muted }
        Text {
            x: 36; anchors.verticalCenter: parent.verticalCenter
            text: nav.module.name
            color: Theme.text
            font { family: Theme.font; pointSize: Theme.pt(12.5); weight: nav.within ? Font.Medium : Font.Normal }
        }
        Text {
            anchors { right: parent.right; rightMargin: 12; verticalCenter: parent.verticalCenter }
            visible: text !== ""
            text: SettingsState.badges[nav.module.id] ?? ""
            color: Theme.rose
            font { family: Theme.mono; pointSize: Theme.pt(10.5) }
        }
        MouseArea { id: navMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: SettingsState.open(nav.module.id) }
    }

    component SubNavItem: Rectangle {
        id: subnav
        property var module
        property var pageInfo
        readonly property bool on: SettingsState.page === module.id && SettingsState.sub === pageInfo.id
        Accessible.role: Accessible.PageTab
        Accessible.name: pageInfo.name
        Accessible.selected: on
        Accessible.onPressAction: SettingsState.open(module.id + "/" + pageInfo.id)
        width: parent?.width ?? 0
        height: 30
        radius: 8
        color: on ? Theme.surface2 : subMouse.containsMouse ? Theme.surface1 : "transparent"
        Icon { x: 30; anchors.verticalCenter: parent.verticalCenter; name: subnav.pageInfo.icon ?? "chevR"; size: 13; color: subnav.on ? Theme.rose : Theme.faint }
        Text {
            x: 52; anchors.verticalCenter: parent.verticalCenter
            text: subnav.pageInfo.name
            color: subnav.on ? Theme.text : Theme.muted
            font { family: Theme.font; pointSize: Theme.pt(12); weight: subnav.on ? Font.Medium : Font.Normal }
        }
        MouseArea { id: subMouse; anchors.fill: parent; hoverEnabled: true; cursorShape: Qt.PointingHandCursor; onClicked: SettingsState.open(subnav.module.id + "/" + subnav.pageInfo.id) }
    }

    // The open page: loaded on entry, unloaded on leaving (Modules page).
    // A page can set `footer` (a Component) for a bar fixed at the bottom,
    // e.g. Apply / Revert on Displays.
    // A question asked in place (Dialog with strip): across the top of the page.
    Column {
        id: stripHost
        anchors { left: win.navEdge.right; leftMargin: win.pad; right: parent.right; rightMargin: win.pad; top: parent.top; topMargin: 40 }
        Component.onCompleted: SettingsState.stripHost = stripHost
    }
    Flickable {
        id: scroller
        // Starts below the window buttons, so the page scrolls away under
        // that strip instead of under the buttons.
        anchors { left: win.navEdge.right; right: parent.right; top: SettingsState.asking?.strip && SettingsState.asking?.isOpen ? stripHost.bottom : parent.top; topMargin: SettingsState.asking?.strip && SettingsState.asking?.isOpen ? 4 : 36; bottom: footerBar.visible ? footerBar.top : parent.bottom }
        contentHeight: pageLoader.height + 50
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        Loader {
            id: pageLoader
            // (The page area starts below the window buttons.)
            x: win.pad; y: 14
            width: scroller.width - 2 * win.pad
            source: SettingsState.current ? "file://" + SettingsState.current.dir + "/" + (SettingsState.currentSub?.file ?? "Page.qml") : ""
            onSourceChanged: scroller.contentY = 0
            onStatusChanged: if (status === Loader.Error) console.warn("settings: page failed to load:", source)
        }
    }
    Rectangle {
        id: footerBar
        visible: (pageLoader.item?.footer ?? null) !== null
        anchors { left: win.navEdge.right; right: parent.right; bottom: parent.bottom }
        height: visible ? 54 : 0
        color: Theme.bg
        Loader {
            anchors { fill: parent; leftMargin: win.pad; rightMargin: win.pad }
            sourceComponent: pageLoader.item?.footer ?? null
        }
    }

    // Something to fill in, or a thing's own settings (Dialog): its own
    // page over this one, with ‹ back.
    Item {
        id: sheetHost
        anchors { left: win.navEdge.right; right: parent.right; top: parent.top; topMargin: 36; bottom: parent.bottom }
        z: 5
        Component.onCompleted: SettingsState.sheetHost = sheetHost
    }

    // Window buttons, top right: minimise, maximise (restore), close.
    // The strip beside them moves the window; double-click maximises.
    MouseArea {
        anchors { left: win.navEdge.right; right: windowButtons.left; top: parent.top }
        height: 24
        onPressed: win.startSystemMove()
        onDoubleClicked: win.maximized = !win.maximized
    }
    Row {
        id: windowButtons
        anchors { top: parent.top; right: parent.right }
        Repeater {
            model: [
                { icon: "minus", label: "Minimise", run: () => win.minimized = true },
                { icon: win.maximized ? "restore" : "square", label: win.maximized ? "Restore" : "Maximise", run: () => win.maximized = !win.maximized },
                { icon: "x", label: "Close", close: true, run: () => Qt.quit() },
            ]
            delegate: Rectangle {
                required property var modelData
                Accessible.role: Accessible.Button
                Accessible.name: modelData.label
                Accessible.onPressAction: modelData.run()
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

    // "/" anywhere jumps to search (narrow: in the sidebar, slid over).
    Shortcut { sequence: "/"; onActivated: win.narrow ? win.openDrawer(true) : search.forceActiveFocus() }
    Shortcut { sequence: "Escape"; enabled: win.drawerOpen; onActivated: win.drawerOpen = false }
    Shortcut { sequence: "Ctrl+Q"; onActivated: Qt.quit() }
    Shortcut { sequence: "Ctrl+W"; onActivated: Qt.quit() }

}
