import QtQuick
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: keeps

    required property var swarm
    readonly property real r: swarm.ratio
    readonly property var list: swarm.info?.keeps ?? []

    function patch(e, change) {
        const v = swarm.forceOf(e.index);
        change(v[e.kind]);
        swarm.edit("force", {
                "index": e.index,
                "force": v
            });
    }

    onListChanged: art.requestPaint()
    onRChanged: art.requestPaint()

    Canvas {
        id: art

        anchors.fill: parent
        onPaint: {
            const ctx = getContext("2d"), r = keeps.r;
            ctx.reset();
            ctx.strokeStyle = Kirigami.Theme.linkColor;
            ctx.lineWidth = 1.5;
            ctx.setLineDash([3, 3]);
            keeps.list.forEach(e => {
                    ctx.beginPath();
                    if (e.kind === "keep_distance" && e.at)
                        ctx.arc(e.at[0] * r, e.at[1] * r, e.radius * r, 0, 2 * Math.PI);
                    if (e.kind === "keep_between" && e.from && e.to) {
                        ctx.moveTo(e.from[0] * r, e.from[1] * r);
                        ctx.lineTo(e.to[0] * r, e.to[1] * r);
                    }
                    ctx.stroke();
                });
        }
    }

    Repeater {
        model: keeps.list

        delegate: Item {
            id: keep

            required property var modelData

            anchors.fill: parent

            Repeater {
                model: SceneSwarmGrips.keeps(keeps, keep.modelData)

                delegate: SceneKnob {
                    required property var modelData

                    at: modelData.at
                    ratio: keeps.r
                    label: modelData.label ?? ""
                    tint: Kirigami.Theme.linkColor
                    tip: modelData.tip ?? App.tr("Drag to move the point kept to")
                    onMoved: (to, done) => modelData.act(to)
                }
            }
        }
    }
}
