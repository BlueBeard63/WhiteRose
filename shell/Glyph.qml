// The bar's icons (Figma: Components › "Bar Icons — 16px"): solid glyphs on
// a 16px grid, drawn 1:1 so their straight edges land on whole pixels at
// scale 1 and 2. (The Lucide outlines in Icon.qml are drawn from 24px and
// shrunk, which goes soft at the bar's size.) Each glyph has a fill, parts
// drawn dim (the bands a weak signal doesn't reach) and 2px lines. Keep
// the item on whole pixels too, e.g. y: Math.round((parent.height - height) / 2).
import QtQuick
import QtQuick.Shapes

Item {
    id: root
    property string name
    property color color: Theme.text
    readonly property var glyph: glyphs[name] ?? ({})

    implicitWidth: glyph.w ?? 16
    implicitHeight: 16

    readonly property var glyphs: ({
        wifi: { fill: "M8 13.5L5.879 11.379A3 3 0 0 1 10.121 11.379ZM3.05 8.55A7 7 0 0 1 12.95 8.55L11.182 10.318A4.5 4.5 0 0 0 4.818 10.318ZM0.222 5.722A11 11 0 0 1 15.778 5.722L14.01 7.49A8.5 8.5 0 0 0 1.99 7.49Z" },
        wifiMid: { dim: "M0.222 5.722A11 11 0 0 1 15.778 5.722L14.01 7.49A8.5 8.5 0 0 0 1.99 7.49Z", fill: "M8 13.5L5.879 11.379A3 3 0 0 1 10.121 11.379ZM3.05 8.55A7 7 0 0 1 12.95 8.55L11.182 10.318A4.5 4.5 0 0 0 4.818 10.318Z" },
        wifiLow: { dim: "M3.05 8.55A7 7 0 0 1 12.95 8.55L11.182 10.318A4.5 4.5 0 0 0 4.818 10.318ZM0.222 5.722A11 11 0 0 1 15.778 5.722L14.01 7.49A8.5 8.5 0 0 0 1.99 7.49Z", fill: "M8 13.5L5.879 11.379A3 3 0 0 1 10.121 11.379Z" },
        wifiOff: { dim: "M8 13.5L5.879 11.379A3 3 0 0 1 10.121 11.379ZM3.05 8.55A7 7 0 0 1 12.95 8.55L11.182 10.318A4.5 4.5 0 0 0 4.818 10.318ZM0.222 5.722A11 11 0 0 1 15.778 5.722L14.01 7.49A8.5 8.5 0 0 0 1.99 7.49Z", stroke: "M2.5 2.5L13.5 13.5" },
        ethernet: { fill: "M6 1H10A1 1 0 0 1 11 2V5A1 1 0 0 1 10 6H6A1 1 0 0 1 5 5V2A1 1 0 0 1 6 1ZM7 6H9V8H7ZM3 8H13V9H3ZM3 9H4V10H3ZM12 9H13V10H12ZM2 10H5A1 1 0 0 1 6 11V14A1 1 0 0 1 5 15H2A1 1 0 0 1 1 14V11A1 1 0 0 1 2 10ZM11 10H14A1 1 0 0 1 15 11V14A1 1 0 0 1 14 15H11A1 1 0 0 1 10 14V11A1 1 0 0 1 11 10Z" },
        bluetooth: { stroke: "M4 4.5L12 11L8 14.5V1.5L12 5L4 11.5" },
        volume: { fill: "M1 6H4L8 2V14L4 10H1Z", stroke: "M10.5 6A2.5 2.5 0 0 1 10.5 10M12 3.5A5.5 5.5 0 0 1 12 12.5" },
        volumeMuted: { fill: "M1 6H4L8 2V14L4 10H1Z", stroke: "M11 6L15 10M15 6L11 10" },
        battery: { fill: "M2 3H12A2 2 0 0 1 14 5V11A2 2 0 0 1 12 13H2A2 2 0 0 1 0 11V5A2 2 0 0 1 2 3ZM2 5H12V11H2ZM14 6H15A1 1 0 0 1 16 7V9A1 1 0 0 1 15 10H14Z" },
        zap: { w: 8, fill: "M5.5 2L1 9H4L3 14L7.5 7H4.5Z" },
        bell: { fill: "M8 1.5C5.24 1.5 3.5 3.57 3.5 6.25V9.25L2 11.25V12.5H14V11.25L12.5 9.25V6.25C12.5 3.57 10.76 1.5 8 1.5ZM6 13.5H10A2 2 0 0 1 6 13.5Z" },
        moon: { fill: "M9.5 1.5A6.5 6.5 0 1 0 14.5 11A5.25 5.25 0 0 1 9.5 1.5Z" },
        layoutFloating: { fill: "M2 2H9A1 1 0 0 1 10 3V5H5A1 1 0 0 0 4 6V9H2A1 1 0 0 1 1 8V3A1 1 0 0 1 2 2ZM6 6H14A1 1 0 0 1 15 7V13A1 1 0 0 1 14 14H6A1 1 0 0 1 5 13V7A1 1 0 0 1 6 6Z" },
        layoutDwindle: { fill: "M2 2H6A1 1 0 0 1 7 3V13A1 1 0 0 1 6 14H2A1 1 0 0 1 1 13V3A1 1 0 0 1 2 2ZM9 2H14A1 1 0 0 1 15 3V6A1 1 0 0 1 14 7H9A1 1 0 0 1 8 6V3A1 1 0 0 1 9 2ZM9 8H10A1 1 0 0 1 11 9V13A1 1 0 0 1 10 14H9A1 1 0 0 1 8 13V9A1 1 0 0 1 9 8ZM13 8H14A1 1 0 0 1 15 9V13A1 1 0 0 1 14 14H13A1 1 0 0 1 12 13V9A1 1 0 0 1 13 8Z" },
        layoutMaster: { fill: "M2 2H8A1 1 0 0 1 9 3V13A1 1 0 0 1 8 14H2A1 1 0 0 1 1 13V3A1 1 0 0 1 2 2ZM11 2H14A1 1 0 0 1 15 3V6A1 1 0 0 1 14 7H11A1 1 0 0 1 10 6V3A1 1 0 0 1 11 2ZM11 8H14A1 1 0 0 1 15 9V13A1 1 0 0 1 14 14H11A1 1 0 0 1 10 13V9A1 1 0 0 1 11 8Z" },
    })

    Shape {
        anchors.fill: parent
        preferredRendererType: Shape.CurveRenderer

        ShapePath {
            fillColor: Qt.rgba(root.color.r, root.color.g, root.color.b, root.color.a * 0.35)
            fillRule: ShapePath.OddEvenFill
            strokeColor: "transparent"
            PathSvg { path: root.glyph.dim ?? "" }
        }
        ShapePath {
            fillColor: root.color
            fillRule: ShapePath.OddEvenFill
            strokeColor: "transparent"
            PathSvg { path: root.glyph.fill ?? "" }
        }
        ShapePath {
            fillColor: "transparent"
            strokeColor: root.glyph.stroke ? root.color : "transparent"
            strokeWidth: 2
            capStyle: ShapePath.RoundCap
            joinStyle: ShapePath.RoundJoin
            PathSvg { path: root.glyph.stroke ?? "" }
        }
    }
}
