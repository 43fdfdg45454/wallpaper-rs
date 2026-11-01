import QtQuick
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: shape

    required property var swarm
    required property var emitter
    readonly property real r: swarm.ratio
    readonly property var knobs: SceneSwarmKnobs.of(shape)

    function set(next) {
        swarm.edit("shape", {
            "emitter": emitter.index,
            "shape": next
        });
    }

    function patch(change) {
        const s = swarm.shapeOf(emitter.index);
        change(s[emitter.kind] ?? s);
        set(s);
    }

    function mark(ctx, p, size) {
        ctx.beginPath();
        ctx.arc(p[0] * r, p[1] * r, Math.max(1.5, 3 * size), 0, 2 * Math.PI);
        ctx.fill();
    }

    onEmitterChanged: art.requestPaint()
    onRChanged: art.requestPaint()

    Canvas {
        id: art

        anchors.fill: parent
        onPaint: {
            const ctx = getContext("2d"), e = shape.emitter, r = shape.r;
            ctx.reset();
            ctx.strokeStyle = Kirigami.Theme.highlightColor;
            ctx.fillStyle = Kirigami.Theme.highlightColor;
            ctx.lineWidth = 2;
            const path = (list, closed) => {
                ctx.beginPath();
                list.filter(p => p).forEach((p, i) => i ? ctx.lineTo(p[0] * r, p[1] * r) : ctx.moveTo(p[0] * r, p[1] * r));
                if (closed)
                    ctx.closePath();
                ctx.stroke();
            };
            if (e.kind === "box")
                path(e.corners, true);
            if (e.kind === "line")
                path([e.from, e.to], false);
            if (e.kind === "circle") {
                ctx.beginPath();
                ctx.arc(e.center[0] * r, e.center[1] * r, e.radius * r, 0, 2 * Math.PI);
                ctx.stroke();
            }
            if (e.kind === "around" && e.center) {
                const a = e.start * Math.PI / 180;
                ctx.beginPath();
                ctx.arc(e.center[0] * r, e.center[1] * r, e.radius * r, a, a + e.arc * Math.PI / 180);
                ctx.stroke();
                e.places.forEach(p => shape.mark(ctx, p, 1));
            }
            if (e.kind === "between") {
                path(e.places.map(p => p.at), false);
                e.places.forEach(p => shape.mark(ctx, p.at, p.size));
            }
            if (e.kind === "layer") {
                ctx.setLineDash([4, 4]);
                path(e.outline, true);
                ctx.globalAlpha = 0.6;
                e.samples.forEach(p => shape.mark(ctx, p, 0.6));
            }
        }
    }

    Repeater {
        model: shape.knobs

        delegate: SceneKnob {
            required property var modelData

            at: modelData.at
            ratio: shape.r
            label: modelData.label ?? ""
            tip: modelData.tip ?? ""
            onMoved: (to, done) => modelData.act(to)
        }
    }
}
