import QtQuick
import org.kde.kirigami as Kirigami

Canvas {
    id: lines

    required property real ratio
    property var segments: []
    property color tint: Kirigami.Theme.neutralTextColor
    property bool dashed

    onSegmentsChanged: requestPaint()
    onRatioChanged: requestPaint()
    onPaint: {
        const ctx = getContext("2d");
        ctx.reset();
        ctx.strokeStyle = tint;
        ctx.lineWidth = 1.5;
        ctx.setLineDash(dashed ? [4, 4] : []);
        lines.segments.forEach(s => {
            ctx.beginPath();
            ctx.moveTo(s[0][0] * ratio, s[0][1] * ratio);
            ctx.lineTo(s[1][0] * ratio, s[1][1] * ratio);
            ctx.stroke();
        });
    }
}
