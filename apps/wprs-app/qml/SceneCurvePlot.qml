import QtQuick
import org.kde.kirigami as Kirigami

Canvas {
    id: plot

    property var curve: null
    property var marks: []
    readonly property var points: curve?.points ?? []

    function spot(p) {
        const x = curve.x, y = curve.y;
        return [(p[0] - x[0]) / Math.max(1e-9, x[1] - x[0]) * width, (1 - (p[1] - y[0]) / Math.max(1e-9, y[1] - y[0])) * height];
    }

    implicitHeight: Kirigami.Units.gridUnit * 4
    onCurveChanged: requestPaint()
    onMarksChanged: requestPaint()
    onWidthChanged: requestPaint()
    onPaint: {
        const ctx = getContext("2d");
        ctx.reset();
        ctx.strokeStyle = Kirigami.Theme.disabledTextColor;
        ctx.strokeRect(0, 0, width, height);
        if (!curve || points.length < 2)
            return;
        ctx.setLineDash([3, 3]);
        plot.marks.forEach(m => {
            const s = plot.spot([m, curve.y[0]]);
            ctx.beginPath();
            ctx.moveTo(s[0], 0);
            ctx.lineTo(s[0], height);
            ctx.stroke();
        });
        ctx.setLineDash([]);
        ctx.strokeStyle = Kirigami.Theme.highlightColor;
        ctx.lineWidth = 2;
        ctx.beginPath();
        points.forEach((p, i) => {
            const s = plot.spot(p);
            i ? ctx.lineTo(s[0], s[1]) : ctx.moveTo(s[0], s[1]);
        });
        ctx.stroke();
    }
}
