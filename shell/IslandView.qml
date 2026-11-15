// One view of the centre island, declared in its own file in shell/island/
// (the file's name is the view's name: island/clipboard.qml is
// ShellState.show("clipboard")). Put its content inside; the island takes
// the content's size and morphs to fit it, and back to rest when it closes.
//
//   IslandView {
//       keyboard: true          // takes the keys; Esc or clicking away closes it
//       MyContent {}            // sized by its implicitWidth / implicitHeight
//   }
//
// What else a view can say about itself:
//   keyboard   takes the keyboard (default true). Without it, clicking
//              elsewhere doesn't close it either: for things driven from
//              outside (the ALT+TAB switcher) or questions that stay until
//              they're answered (the "couldn't start" prompt).
//   overhang   room it needs below the island (the dock's window previews),
//              drawn outside the outline instead of clipped to it.
//   inputs     items outside the outline that still take clicks (those
//              previews).
//   bleed      true: it may draw into the island's left ear, left of its
//              own x = 0 (the music cover); everything else is clipped to
//              the island's body.
import QtQuick

Item {
    id: view
    default property alias content: holder.data
    property bool keyboard: true
    property real overhang: 0
    property list<Item> inputs
    property bool bleed: false
    // The content itself (the first item put inside).
    readonly property Item item: holder.children.length > 0 ? holder.children[0] : null

    implicitWidth: view.item ? view.item.implicitWidth : 0
    implicitHeight: view.item ? view.item.implicitHeight : 0
    Item {
        id: holder
        width: view.width
        height: view.height
    }
    // The content fills the view (as it did loaded on its own).
    Binding { target: view.item; property: "width"; value: view.width; when: view.item !== null }
    Binding { target: view.item; property: "height"; value: view.height; when: view.item !== null }
}
