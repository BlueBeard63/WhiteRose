// A file's state in a folder a sync client keeps (cloudsync.rs): synced,
// syncing, waiting to upload, couldn't sync, or only in the cloud.
import QtQuick
import QtQuick.Controls.Basic
import WhiteRose.Kit

Rectangle {
    id: root
    property string state: ""
    property real size: 14
    readonly property var look: ({
        synced: { icon: "check", colour: "#57AB5A", tip: "Synced" },
        syncing: { icon: "restart", colour: "#539BF5", tip: "Syncing" },
        new: { icon: "chevUp", colour: "#539BF5", tip: "Waiting to upload" },
        error: { icon: "alert", colour: "#E5534B", tip: "Couldn't sync" },
        online: { icon: "cloud", colour: Theme.muted, tip: "Only in the cloud: opening it downloads it" },
    })[root.state] ?? null
    visible: root.look !== null
    width: root.size; height: root.size; radius: root.size / 2
    color: Theme.bg
    border { width: 1; color: root.look?.colour ?? "transparent" }
    Icon { anchors.centerIn: parent; name: root.look?.icon ?? "check"; size: root.size * 0.62; color: root.look?.colour ?? "transparent" }
    HoverHandler { id: hover }
    ToolTip.visible: hover.hovered
    ToolTip.delay: 400
    ToolTip.text: root.look?.tip ?? ""
}
