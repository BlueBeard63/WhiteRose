// On/off switch (Figma: rose when on).
import QtQuick


Rectangle {
    id: root
    property bool checked: false
    property bool enabled: true
    // What it switches, in words (usually its row's title).
    property string label: ""
    signal toggled(bool on)
    // For screen readers and the UI tests (whiterose-uitest finds things by
    // their accessible name and role).
    Accessible.role: Accessible.CheckBox
    Accessible.name: root.label
    Accessible.checkable: true
    Accessible.checked: root.checked
    Accessible.onToggleAction: if (root.enabled) root.toggled(!root.checked)
    Accessible.onPressAction: if (root.enabled) root.toggled(!root.checked)
    width: 30; height: 18; radius: 9
    anchors.verticalCenter: parent?.verticalCenter
    color: checked ? Theme.rose : Theme.surface2
    opacity: enabled ? 1 : 0.4
    Behavior on color { ColorAnimation { duration: 140 } }
    Rectangle {
        width: 12; height: 12; radius: 6
        y: 3
        x: root.checked ? root.width - width - 3 : 3
        color: root.checked ? Theme.bg : Theme.muted
        Behavior on x { NumberAnimation { duration: 140; easing.type: Easing.OutCubic } }
    }
    MouseArea {
        anchors.fill: parent
        anchors.margins: -6
        enabled: root.enabled
        cursorShape: Qt.PointingHandCursor
        onClicked: root.toggled(!root.checked)
    }
}
