import QtQuick
import org.kde.kirigami as Kirigami

Item {
    id: handles

    required property var page
    required property var matrix
    readonly property var mask: page.values.masks?.[page.mask] ?? null
    readonly property var points: !mask ? [] : mask.kind === "linear" ? ["from", "to"] : mask.kind === "radial" ? ["from", "rx", "ry"] : []
    readonly property var inverse: {
        const m = matrix, det = m[0][0] * m[1][1] - m[0][1] * m[1][0] || 1;
        const a = m[1][1] / det, b = -m[0][1] / det, c = -m[1][0] / det, d = m[0][0] / det;
        return [[a, b, -(a * m[0][2] + b * m[1][2])], [c, d, -(c * m[0][2] + d * m[1][2])]];
    }

    function apply(m, x, y) {
        return [m[0][0] * x + m[0][1] * y + m[0][2], m[1][0] * x + m[1][1] * y + m[1][2]];
    }

    function spot(pick) {
        const v = mask.values;
        const at = pick === "to" ? [v[2], v[3]] : pick === "rx" ? [v[0] + v[2], v[1]] : pick === "ry" ? [v[0], v[1] + v[3]] : [v[0], v[1]];
        const shown = apply(inverse, at[0], at[1]);
        return Qt.point(shown[0] * width, shown[1] * height);
    }

    function drag(pick, x, y) {
        const at = apply(matrix, x / width, y / height), v = mask.values.slice();
        if (pick === "from" || pick === "to") {
            const i = pick === "to" ? 2 : 0;
            v[i] = at[0];
            v[i + 1] = at[1];
        } else if (pick === "rx") {
            v[2] = Math.abs(at[0] - v[0]);
        } else {
            v[3] = Math.abs(at[1] - v[1]);
        }
        page.run("mask", {
            "name": page.mask,
            "spec": mask.kind + ":" + v.map(n => Math.max(-1, Math.min(2, n)).toFixed(4)).join(","),
            "invert": mask.invert
        });
    }

    onMaskChanged: line.requestPaint()
    onWidthChanged: line.requestPaint()

    Canvas {
        id: line

        anchors.fill: parent
        onPaint: {
            const ctx = getContext("2d");
            ctx.reset();
            if (!handles.mask || handles.points.length === 0)
                return;
            ctx.strokeStyle = Kirigami.Theme.highlightColor;
            ctx.lineWidth = 2;
            ctx.beginPath();
            const v = handles.mask.values;
            const steps = handles.mask.kind === "radial" ? 48 : 1;
            for (let i = 0; i <= steps; i++) {
                const t = 2 * Math.PI * i / steps;
                const at = handles.mask.kind === "radial" ? [v[0] + v[2] * Math.cos(t), v[1] + v[3] * Math.sin(t)] : i === 0 ? [v[0], v[1]] : [v[2], v[3]];
                const p = handles.apply(handles.inverse, at[0], at[1]);
                i === 0 ? ctx.moveTo(p[0] * width, p[1] * height) : ctx.lineTo(p[0] * width, p[1] * height);
            }
            ctx.stroke();
        }
    }

    Repeater {
        model: handles.points

        delegate: Rectangle {
            id: knob

            required property string modelData
            readonly property point at: handles.mask ? handles.spot(modelData) : Qt.point(0, 0)

            x: at.x - width / 2
            y: at.y - height / 2
            width: Kirigami.Units.gridUnit
            height: width
            radius: width / 2
            color: Kirigami.Theme.highlightColor
            border.color: Kirigami.Theme.backgroundColor
            border.width: 2

            MouseArea {
                anchors.fill: parent
                anchors.margins: -Kirigami.Units.smallSpacing
                cursorShape: Qt.SizeAllCursor
                onPositionChanged: mouse => {
                    const p = mapToItem(handles, mouse.x, mouse.y);
                    handles.drag(knob.modelData, p.x, p.y);
                }
            }
        }
    }
}
