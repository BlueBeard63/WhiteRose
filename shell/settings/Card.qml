// A rounded group of settings rows (Figma: the dark cards on each page).
// Clips to its rounded shape, so a row with a background at the top or
// bottom (a highlighted or hovered row) follows the corners.
import QtQuick
import Quickshell.Widgets
import qs

ClippingRectangle {
    id: root
    default property alias content: column.data
    width: parent?.width ?? 0
    implicitHeight: column.implicitHeight
    height: implicitHeight
    radius: 16
    color: Theme.surface1
    Column { id: column; width: parent.width }
}
