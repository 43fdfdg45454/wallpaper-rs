import QtQuick
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: swirl

    required property var swarm
    readonly property real r: swarm.ratio
    readonly property var gust: swarm.info?.fields?.turbulent ?? null
    readonly property var drift: swarm.info?.fields?.offset ?? null
    readonly property real turn: gust ? gust.forward - gust.turbulent.forward : 0
    readonly property real grow: drift && drift.offset.distance > 0 ? drift.radius / drift.offset.distance : 1

    function angle(from, to) {
        return Math.atan2(to[1] - from[1], to[0] - from[0]) * 180 / Math.PI;
    }

    function along(deg, length) {
        const a = deg * Math.PI / 180;
        return [gust.at[0] + Math.cos(a) * length, gust.at[1] + Math.sin(a) * length];
    }

    onGustChanged: art.requestPaint()
    onDriftChanged: art.requestPaint()
    onRChanged: art.requestPaint()

    Canvas {
        id: art

        anchors.fill: parent
        onPaint: {
            const ctx = getContext("2d"), r = swirl.r, g = swirl.gust, d = swirl.drift;
            ctx.reset();
            if (g) {
                const x = g.at[0] * r, y = g.at[1] * r, rad = v => v * Math.PI / 180;
                ctx.fillStyle = Kirigami.Theme.highlightColor;
                ctx.globalAlpha = 0.18;
                ctx.beginPath();
                ctx.moveTo(x, y);
                ctx.arc(x, y, g.reach * r, rad(g.fan[0]), rad(g.fan[1]));
                ctx.closePath();
                ctx.fill();
                ctx.globalAlpha = 0.8;
                ctx.strokeStyle = Kirigami.Theme.highlightColor;
                ctx.lineWidth = 1;
                g.samples.forEach(a => {
                        ctx.beginPath();
                        ctx.moveTo(x, y);
                        ctx.lineTo(x + Math.cos(rad(a)) * g.reach * r * 0.6, y + Math.sin(rad(a)) * g.reach * r * 0.6);
                        ctx.stroke();
                    });
                ctx.lineWidth = 2.5;
                ctx.beginPath();
                ctx.moveTo(x, y);
                ctx.lineTo(g.knob[0] * r, g.knob[1] * r);
                ctx.stroke();
            }
            if (d) {
                ctx.globalAlpha = 0.9;
                ctx.strokeStyle = Kirigami.Theme.positiveTextColor;
                ctx.fillStyle = Kirigami.Theme.positiveTextColor;
                ctx.lineWidth = 1;
                d.arrows.forEach(pair => {
                        ctx.beginPath();
                        ctx.moveTo(pair[0][0] * r, pair[0][1] * r);
                        ctx.lineTo(pair[1][0] * r, pair[1][1] * r);
                        ctx.stroke();
                        ctx.beginPath();
                        ctx.arc(pair[1][0] * r, pair[1][1] * r, 1.8, 0, 2 * Math.PI);
                        ctx.fill();
                    });
                ctx.setLineDash([4, 4]);
                ctx.beginPath();
                ctx.arc(d.at[0] * r, d.at[1] * r, d.radius * r, 0, 2 * Math.PI);
                ctx.stroke();
            }
        }
    }

    SceneKnob {
        at: swirl.gust?.knob ?? null
        ratio: swirl.r
        label: swirl.gust ? App.tr("%1°", Math.round(swirl.gust.turbulent.forward)) : ""
        tip: App.tr("Drag to turn the direction particles start in")
        onMoved: (to, done) => swirl.swarm.edit("turbulent", {
                "turbulent": {
                    "forward": Math.round(((swirl.angle(swirl.gust.at, to) - swirl.turn) % 360 + 540) % 360 - 180)
                }
            })
    }

    SceneKnob {
        at: swirl.gust ? swirl.along(swirl.gust.fan[1], swirl.gust.reach * 0.8) : null
        ratio: swirl.r
        square: true
        tip: App.tr("Drag to widen or narrow how far the noise turns them")
        onMoved: (to, done) => {
            const off = Math.abs(((swirl.angle(swirl.gust.at, to) - swirl.gust.forward) % 360 + 540) % 360 - 180);
            swirl.swarm.edit("turbulent", {
                    "turbulent": {
                        "scale": Math.round(Math.min(1, off / 180) * 100) / 100
                    }
                });
        }
    }

    SceneKnob {
        at: swirl.drift?.knob ?? null
        ratio: swirl.r
        tint: Kirigami.Theme.positiveTextColor
        label: swirl.drift ? App.tr("%1 px", Math.round(swirl.drift.offset.distance)) : ""
        tip: App.tr("Drag to change how far births move")
        onMoved: (to, done) => swirl.swarm.edit("offset", {
                "offset": {
                    "distance": Math.round(Math.hypot(to[0] - swirl.drift.at[0], to[1] - swirl.drift.at[1]) / swirl.grow * 10) / 10
                }
            })
    }
}
