// The black shape every panel is made of: the island, the toast, and later the
// dock and side panels. One closed outline hanging from the bar, with optional
// concave "ears" where it meets the bar and per-corner bottom radii, so a
// panel flush against a screen edge can drop the ear and corner on that side.
//
// Every joint is tangent-continuous. With earHeight + bottom radius equal to
// the height (or less), the ear flows straight into the corner.
import QtQuick
import QtQuick.Shapes

Shape {
    id: root
    property real bodyWidth: 170
    property real bodyHeight: 18
    property bool leftEar: true
    property bool rightEar: true
    property real earWidth: 12
    property real earHeight: 7
    property real bottomLeftRadius: 11
    property real bottomRightRadius: 11
    property color color: Theme.bg

    // Where the body starts inside the shape (after the left ear).
    readonly property real bodyX: leftEar ? earWidth : 0

    width: bodyWidth + (leftEar ? earWidth : 0) + (rightEar ? earWidth : 0)
    height: bodyHeight
    preferredRendererType: Shape.CurveRenderer

    readonly property real kappa: 0.5523
    readonly property string outline: {
        const k = kappa, h = bodyHeight, W = width;
        const ew = earWidth, eh = Math.min(earHeight, h);
        const L = bodyX, R = bodyX + bodyWidth;
        const lEh = leftEar ? eh : 0, rEh = rightEar ? eh : 0;
        const rl = Math.max(0, Math.min(bottomLeftRadius, h - lEh, bodyWidth / 2));
        const rr = Math.max(0, Math.min(bottomRightRadius, h - rEh, bodyWidth / 2));
        const p = [];
        if (leftEar) {
            p.push(`M 0 0`, `C ${ew * k} 0 ${L} ${eh * (1 - k)} ${L} ${eh}`);
        } else {
            p.push(`M ${L} 0`);
        }
        p.push(`L ${L} ${h - rl}`);
        if (rl > 0) p.push(`C ${L} ${h - rl + rl * k} ${L + rl - rl * k} ${h} ${L + rl} ${h}`);
        p.push(`L ${R - rr} ${h}`);
        if (rr > 0) p.push(`C ${R - rr + rr * k} ${h} ${R} ${h - rr + rr * k} ${R} ${h - rr}`);
        if (rightEar) {
            p.push(`L ${R} ${eh}`, `C ${R} ${eh * (1 - k)} ${W - ew * k} 0 ${W} 0`);
        } else {
            p.push(`L ${R} 0`);
        }
        p.push(`Z`);
        return p.join(" ");
    }

    ShapePath {
        strokeWidth: -1
        fillColor: root.color
        PathSvg { path: root.outline }
    }
}
