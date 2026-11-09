// The centre island hanging from the bar (Figma: "Center Island").
//
// Drawn as ONE closed outline so it reads as part of the bar, not a shape
// stuck under it: the edge leaves the bar horizontally, bends down into the
// body's side through a concave fillet, rounds the bottom corner, runs along
// the bottom and mirrors back up. Every joint is tangent-continuous, so there
// are no seams or kinks, and a single fill means no anti-aliasing gaps.
import QtQuick
import QtQuick.Shapes

Item {
    id: root
    property real bodyWidth: 170
    property real bodyHeight: 18
    // Concave fillet where the island meets the bar: how far it sweeps out
    // along the bar, and how far down the body's side it reaches. Keeping it
    // shallow leaves room for generous bottom corners.
    property real earWidth: 12
    property real earHeight: 7
    // Convex corners at the bottom of the body. With earHeight + cornerRadius
    // equal to bodyHeight the ear's curve flows straight into the corner's
    // (an S-bend with no straight segment or kink between them).
    property real cornerRadius: 11
    property color color: Theme.bg
    signal clicked()

    width: bodyWidth + 2 * earWidth
    height: bodyHeight

    // Cubic control-point factor for a quarter circle (or ellipse).
    readonly property real kappa: 0.5523
    readonly property string outline: {
        const ew = earWidth, eh = Math.min(earHeight, bodyHeight), h = bodyHeight;
        const r = Math.min(cornerRadius, h - eh, bodyWidth / 2);
        const W = width, k = kappa;
        const L = ew, R = W - ew;             // body's left and right sides
        return [
            `M 0 0`,
            // Left concave fillet (quarter ellipse): horizontal at the bar,
            // vertical where it meets the body side.
            `C ${ew * k} 0 ${L} ${eh * (1 - k)} ${L} ${eh}`,
            `L ${L} ${h - r}`,
            // Bottom-left convex corner.
            `C ${L} ${h - r + r * k} ${L + r - r * k} ${h} ${L + r} ${h}`,
            `L ${R - r} ${h}`,
            // Bottom-right convex corner.
            `C ${R - r + r * k} ${h} ${R} ${h - r + r * k} ${R} ${h - r}`,
            `L ${R} ${eh}`,
            // Right concave fillet back up to the bar.
            `C ${R} ${eh * (1 - k)} ${W - ew * k} 0 ${W} 0`,
            `Z`
        ].join(" ");
    }

    Shape {
        anchors.fill: parent
        preferredRendererType: Shape.CurveRenderer
        ShapePath {
            strokeWidth: -1
            fillColor: root.color
            PathSvg { path: root.outline }
        }
    }

    // Grip: hints that the island opens the dock.
    Rectangle {
        anchors { horizontalCenter: parent.horizontalCenter; verticalCenter: parent.verticalCenter; verticalCenterOffset: 1 }
        width: 30; height: 3; radius: 2
        color: mouse.containsMouse ? Theme.muted : Theme.grip
        Behavior on color { ColorAnimation { duration: Theme.fastAnim } }
    }

    MouseArea {
        id: mouse
        // Only the body is clickable, not the fillets.
        x: root.earWidth
        width: root.bodyWidth
        height: root.bodyHeight
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.clicked()
    }
}
