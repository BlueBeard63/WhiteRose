// One tab: a folder, or two (F3) side by side or one above the other. The
// side last used is `current`: the toolbar, address bar and keys act on it,
// and a line along its top shows which it is. `initial` is a snapshot
// ({path, back, forward, second, vertical, active}).
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit
import WhiteRose.Files

Item {
    id: tab
    property var initial: ({})
    property bool split: false
    property bool vertical: false
    property int active: 0
    readonly property var second: tab.split ? (secondLoader.item ?? null) : null
    readonly property var panes: tab.second ? [first, tab.second] : [first]
    readonly property var current: tab.active === 1 && tab.second ? tab.second : first
    readonly property var other: tab.second ? (tab.current === first ? tab.second : first) : null
    readonly property string title: tab.current.title

    signal openFile(var entry)
    signal menuAt(var entry, real x, real y)
    signal dropped(var paths, string dest)
    signal changed()

    function focusView() { tab.current.focusView() }
    function refreshView() { for (const p of tab.panes) p.refreshView() }
    // Split (the new side opens where this one is), or back to one: the side in use stays.
    function setSplit(on, start) {
        if (on === tab.split) return;
        if (on) {
            secondLoader.start = start ?? { path: tab.current.path };
            secondLoader.active = true;
            tab.split = true;
            tab.active = 1;
        } else {
            if (tab.active === 1 && secondLoader.item) {
                const s = secondLoader.item.snapshot();
                // The side in use was the second: the tab keeps its folder and history.
                first.back = s.back; first.forward = s.forward; first.path = s.path; first.load();
            }
            tab.split = false;
            tab.active = 0;
            secondLoader.active = false;
        }
        tab.changed();
        Qt.callLater(tab.focusView);
    }
    function setVertical(on) { tab.vertical = on; tab.changed() }
    function snapshot() {
        const s = first.snapshot();
        if (tab.second) {
            s.second = tab.second.snapshot();
            s.vertical = tab.vertical;
            s.active = tab.active;
        }
        return s;
    }
    Component.onCompleted: {
        if (tab.initial.second) {
            tab.vertical = !!tab.initial.vertical;
            tab.setSplit(true, tab.initial.second);
            tab.active = tab.initial.active === 1 ? 1 : 0;
        }
    }

    component Side: Pane {
        onOpenFile: entry => tab.openFile(entry)
        onMenuAt: (entry, x, y) => tab.menuAt(entry, x, y)
        onDropped: (paths, dest) => tab.dropped(paths, dest)
        onChanged: tab.changed()
    }

    SplitView {
        anchors.fill: parent
        orientation: tab.vertical ? Qt.Vertical : Qt.Horizontal
        handle: Item {
            implicitWidth: 7; implicitHeight: 7
            Rectangle {
                anchors.centerIn: parent
                width: tab.vertical ? parent.width : 1
                height: tab.vertical ? 1 : parent.height
                color: SplitHandle.hovered || SplitHandle.pressed ? Theme.muted : Theme.surface2
            }
        }
        Side {
            id: first
            SplitView.fillWidth: true
            SplitView.fillHeight: true
            SplitView.minimumWidth: 200
            SplitView.minimumHeight: 120
            path: tab.initial.path || Fs.home()
            back: tab.initial.back ?? []
            forward: tab.initial.forward ?? []
            onActivated: tab.active = 0
        }
        Loader {
            id: secondLoader
            property var start: ({})
            active: false
            visible: tab.split
            SplitView.preferredWidth: tab.width / 2
            SplitView.preferredHeight: tab.height / 2
            SplitView.minimumWidth: 200
            SplitView.minimumHeight: 120
            sourceComponent: Side {
                path: secondLoader.start.path || Fs.home()
                back: secondLoader.start.back ?? []
                forward: secondLoader.start.forward ?? []
                onActivated: tab.active = 1
            }
        }
    }

    // Which side is in use.
    Rectangle {
        parent: tab.current
        visible: tab.split
        z: 20
        width: parent.width; height: 2
        color: Theme.tint(0.8)
    }
}
