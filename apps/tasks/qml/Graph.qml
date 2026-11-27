import QtQuick
import WhiteRose.Kit

Canvas {
    id: graph
    property var values: []
    property var other: []
    property real ceiling: 100
    property color lineColor: Theme.rose
    property color otherColor: Theme.pearl
    property color gridColor: Theme.surface2
    onValuesChanged: requestPaint()
    onOtherChanged: requestPaint()
    onCeilingChanged: requestPaint()
    onLineColorChanged: requestPaint()
    onOtherColorChanged: requestPaint()
    onGridColorChanged: requestPaint()
    onWidthChanged: requestPaint()
    onHeightChanged: requestPaint()
    onPaint: {
        const c = getContext("2d");
        c.clearRect(0, 0, width, height);
        c.strokeStyle = gridColor; c.lineWidth = 1;
        c.beginPath();
        for (let i = 1; i < 4; i++) { c.moveTo(0, height*i/4); c.lineTo(width, height*i/4); }
        for (let i = 1; i < 6; i++) { c.moveTo(width*i/6, 0); c.lineTo(width*i/6, height); }
        c.stroke();
        function draw(v, color) {
            c.strokeStyle = color; c.lineWidth = 1.6; c.beginPath();
            let started = false;
            v.forEach((n,i) => {
                if (n === null || n === undefined) { started = false; return; }
                const x = width * (60 - v.length + i) / 59, y = height * (1 - Math.max(0, Math.min(1, n / Math.max(1, ceiling))));
                if (started) c.lineTo(x,y); else c.moveTo(x,y);
                started = true;
            });
            c.stroke();
        }
        draw(values,lineColor); draw(other,otherColor);
    }
}
