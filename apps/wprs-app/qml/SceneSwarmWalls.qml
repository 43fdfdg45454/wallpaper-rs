import QtQuick
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: walls

    required property var swarm
    readonly property real r: swarm.ratio
    readonly property var list: swarm.info?.colliders ?? []

    function patch(c, change) {
        const v = JSON.parse(JSON.stringify(c.value));
        change(v.bounds[c.kind]);
        swarm.edit("collider", {
                "index": c.index,
                "collider": v
            });
    }

    function tint(c) {
        if (c.response === "kill")
            return Kirigami.Theme.negativeTextColor;
        return c.response === "stick" ? Kirigami.Theme.positiveTextColor : Kirigami.Theme.neutralTextColor;
    }

    onListChanged: art.requestPaint()
    onRChanged: art.requestPaint()

    Canvas {
        id: art

        anchors.fill: parent
        onPaint: {
            const ctx = getContext("2d"), r = walls.r;
            ctx.reset();
            const line = (list, closed) => {
                ctx.beginPath();
                list.filter(p => p).forEach((p, i) => i ? ctx.lineTo(p[0] * r, p[1] * r) : ctx.moveTo(p[0] * r, p[1] * r));
                if (closed)
                    ctx.closePath();
                ctx.stroke();
            };
            walls.list.forEach(c => {
                    ctx.strokeStyle = walls.tint(c);
                    ctx.lineWidth = 2;
                    ctx.setLineDash(c.inside ? [6, 4] : []);
                    if (c.kind === "canvas" || c.kind === "box")
                        line(c.corners, true);
                    if (c.kind === "plane") {
                        line(c.line, false);
                        line([c.point, c.tip], false);
                    }
                    if (c.kind === "segment")
                        line([c.from, c.to], false);
                    if (c.kind === "circle") {
                        ctx.beginPath();
                        ctx.arc(c.center[0] * r, c.center[1] * r, c.radius * r, 0, 2 * Math.PI);
                        ctx.stroke();
                    }
                    if (c.kind === "layer") {
                        ctx.setLineDash([4, 4]);
                        line(c.outline, true);
                        c.field.forEach(f => {
                                ctx.globalAlpha = Math.max(0.1, 0.8 - Math.abs(f[2]) / 40);
                                ctx.fillStyle = f[2] < 0 ? walls.tint(c) : Kirigami.Theme.disabledTextColor;
                                ctx.fillRect(f[0] * r - 2, f[1] * r - 2, 4, 4);
                            });
                        ctx.globalAlpha = 1;
                    }
                });
        }
    }

    Repeater {
        model: walls.list

        delegate: Item {
            id: wall

            required property var modelData

            anchors.fill: parent

            Repeater {
                model: SceneSwarmGrips.walls(walls, wall.modelData)

                delegate: SceneKnob {
                    required property var modelData

                    at: modelData.at
                    ratio: walls.r
                    square: true
                    tint: walls.tint(wall.modelData)
                    tip: modelData.tip ?? App.tr("Drag to move the collider")
                    onMoved: (to, done) => modelData.act(to)
                }
            }
        }
    }
}
