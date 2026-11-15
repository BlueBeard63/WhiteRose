// A folder with a look of its own (Files › Colour and icon): drawn in its
// colour with its symbol on the front, or its picture. `look` is
// {colour, symbol, image}; `size` the square it fills.
import QtQuick
import QtQuick.Shapes
import WhiteRose.Kit
import "Format.js" as Fmt

Item {
    id: root
    property var look: ({})
    property real size: 48
    width: size; height: size

    // The palette (names as stored: looks.rs).
    readonly property var palette: ({
        rose: "#E8A3AE", red: "#E5534B", orange: "#F0883E", yellow: "#E3B341", green: "#57AB5A",
        teal: "#39C5BB", blue: "#539BF5", purple: "#986EE2", pink: "#E275AD", grey: "#8B949E",
    })
    readonly property color colour: root.palette[root.look?.colour] ?? "#6CA0DC"
    readonly property bool picture: !!root.look?.image && pic.status !== Image.Error

    Image {
        id: pic
        anchors.fill: parent
        visible: root.picture
        source: root.look?.image ? Fmt.fileUrl(root.look.image) : ""
        sourceSize: Qt.size(root.size * 2, root.size * 2)
        fillMode: Image.PreserveAspectFit
        asynchronous: true
    }

    Shape {
        visible: !root.picture
        width: 24; height: 24
        scale: root.size / 24
        transformOrigin: Item.TopLeft
        preferredRendererType: Shape.CurveRenderer
        // The back, with its tab, a shade darker.
        ShapePath {
            strokeWidth: -1
            fillColor: Qt.darker(root.colour, 1.35)
            PathSvg { path: "M2 6.5a2 2 0 0 1 2-2h4.6a2 2 0 0 1 1.5.7l1.3 1.5h8.6a2 2 0 0 1 2 2V18a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2z" }
        }
        // The front.
        ShapePath {
            strokeWidth: -1
            fillColor: root.colour
            PathSvg { path: "M2 10a1.6 1.6 0 0 1 1.6-1.6h16.8A1.6 1.6 0 0 1 22 10v8a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2z" }
        }
    }
    // The symbol, on the front.
    Icon {
        visible: !root.picture && !!root.look?.symbol
        x: root.size * (12 / 24) - width / 2
        y: root.size * (14.4 / 24) - height / 2
        name: root.look?.symbol ?? ""
        size: Math.max(7, root.size * 0.3)
        color: Qt.rgba(1, 1, 1, 0.92)
    }
}
