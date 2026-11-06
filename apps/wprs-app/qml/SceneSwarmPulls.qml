import QtQuick
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: pulls

    required property var swarm
    readonly property real r: swarm.ratio
    readonly property var list: (swarm.info?.pulls ?? []).filter(e => Array.isArray(e.at))

    function patch(e, change) {
        const v = swarm.forceOf(e.index);
        change(v[e.kind]);
        swarm.edit("force", {
                "index": e.index,
                "force": v
            });
    }

    function reach(e, ring, to) {
        const unit = ring.value > 0 ? ring.px / ring.value : 1;
        const d = Math.round(Math.hypot(to[0] - e.at[0], to[1] - e.at[1]) / Math.max(1e-6, unit) * 10) / 10;
        patch(e, v => {
                if (ring.key === "ring")
                    v.ring.radius = d;
                else
                    v[ring.key] = d;
            });
    }

    onListChanged: art.requestPaint()
    onRChanged: art.requestPaint()

    Canvas {
        id: art

        anchors.fill: parent
        onPaint: {
            const ctx = getContext("2d"), r = pulls.r;
            ctx.reset();
            ctx.strokeStyle = Kirigami.Theme.neutralTextColor;
            ctx.lineWidth = 1.5;
            ctx.setLineDash([4, 3]);
            pulls.list.forEach(e => e.rings.forEach(ring => {
                            ctx.beginPath();
                            ctx.arc(e.at[0] * r, e.at[1] * r, ring.px * r, 0, 2 * Math.PI);
                            ctx.stroke();
                        }));
        }
    }

    Repeater {
        model: pulls.list

        delegate: Item {
            id: pull

            required property var modelData

            anchors.fill: parent

            SceneKnob {
                at: pull.modelData.at
                ratio: pulls.r
                label: SceneWords.value(pull.modelData.kind)
                tint: Kirigami.Theme.neutralTextColor
                tip: App.tr("Drag to move the center of the force")
                onMoved: (to, done) => SceneSwarmGrips.spot(pulls, pull.modelData, pull.modelData.ref, to, (v, p) => v[pull.modelData.field] = p)
            }

            Repeater {
                model: pull.modelData.rings

                delegate: SceneKnob {
                    required property var modelData

                    at: modelData.at
                    ratio: pulls.r
                    square: true
                    label: SceneWords.label(modelData.key)
                    tint: Kirigami.Theme.neutralTextColor
                    tip: App.tr("Drag to change how far the force reaches")
                    onMoved: (to, done) => pulls.reach(pull.modelData, modelData, to)
                }
            }
        }
    }
}
