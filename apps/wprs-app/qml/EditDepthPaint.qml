import QtQuick
import org.kde.kirigami as Kirigami

Item {
    id: paint

    required property var page
    readonly property var brush: page.paint
    readonly property bool hot: !(brush?.second ?? false)
    readonly property int most: 1000
    readonly property real size: (brush?.radius ?? 0) * Math.max(width, height)
    property var stroke: []
    property point at: Qt.point(-1, -1)

    function add(x, y) {
        const to = [x / width, y / height].map(n => Math.max(0, Math.min(1, n)));
        const last = stroke[stroke.length - 1];
        if (last && Math.hypot(to[0] - last[0], to[1] - last[1]) <= Math.max(brush.radius / 4, 0.003))
            return;
        stroke = stroke.concat([to]);
        if (stroke.length >= most) {
            lay();
            stroke = [to];
        }
        line.requestPaint();
    }

    function lay() {
        const b = brush, points = stroke;
        stroke = [];
        line.requestPaint();
        if (!b || points.length === 0)
            return;
        const list = points.map(p => p[0].toFixed(4) + "," + p[1].toFixed(4)).join(";");
        if (b.kind === "fill")
            page.run("fill_mask", {
                    "points": list,
                    "radius": b.radius,
                    "falloff": b.falloff,
                    "refill": !b.second,
                    "protect": b.second
                });
        else
            page.run("depth_brush", {
                    "points": list,
                    "by": b.by,
                    "radius": b.radius,
                    "falloff": b.falloff,
                    "raise": !b.second,
                    "lower": b.second
                });
    }

    Canvas {
        id: line

        anchors.fill: parent
        onPaint: {
            const ctx = getContext("2d");
            ctx.reset();
            if (paint.stroke.length === 0)
                return;
            ctx.strokeStyle = paint.hot ? Kirigami.Theme.highlightColor : Kirigami.Theme.negativeTextColor;
            ctx.globalAlpha = 0.5;
            ctx.lineCap = "round";
            ctx.lineJoin = "round";
            ctx.lineWidth = Math.max(2, 2 * paint.size);
            ctx.beginPath();
            paint.stroke.forEach((p, i) => {
                    i === 0 ? ctx.moveTo(p[0] * width, p[1] * height) : ctx.lineTo(p[0] * width, p[1] * height);
                });
            ctx.stroke();
        }
    }

    Rectangle {
        x: paint.at.x - paint.size
        y: paint.at.y - paint.size
        width: 2 * paint.size
        height: width
        radius: paint.size
        visible: paint.at.x >= 0 && paint.brush !== null
        color: "transparent"
        border.width: 1
        border.color: paint.hot ? Kirigami.Theme.highlightColor : Kirigami.Theme.negativeTextColor
    }

    MouseArea {
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.CrossCursor
        onPressed: mouse => paint.add(mouse.x, mouse.y)
        onPositionChanged: mouse => {
            paint.at = Qt.point(mouse.x, mouse.y);
            if (pressed)
                paint.add(mouse.x, mouse.y);
        }
        onReleased: paint.lay()
        onCanceled: paint.lay()
        onExited: paint.at = Qt.point(-1, -1)
    }
}
