// Gives its window a title: the layer's namespace ("whiterose-bar"). Layer
// windows have none of their own, and screen readers (and the UI tests,
// which find the bar's things through the accessibility tree) know a
// window by its title.
import QtQuick

Item {
    id: root
    required property string name
    visible: false
    function apply() { if (root.Window.window) root.Window.window.title = root.name }
    Window.onWindowChanged: apply()
    onNameChanged: apply()
    Component.onCompleted: apply()
}
