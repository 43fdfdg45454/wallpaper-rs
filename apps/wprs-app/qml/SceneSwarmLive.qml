import QtQuick
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: live

    required property var swarm
    readonly property real r: swarm.ratio
    readonly property var census: SceneSwarmCensus.census?.system === swarm.system ? SceneSwarmCensus.census : null
    readonly property var dots: census?.particles ?? []
    readonly property real reach: swarm.info?.boids?.radius ?? 0
    readonly property var picked: dots.find(p => p[2] === SceneSwarmCensus.picked) ?? null

    function near(p) {
        return picked !== null && p !== picked && Math.hypot(p[0] - picked[0], p[1] - picked[1]) < reach;
    }

    visible: SceneSwarmCensus.shown && census !== null
    onCensusChanged: art.requestPaint()
    onPickedChanged: art.requestPaint()
    onRChanged: art.requestPaint()

    Canvas {
        id: art

        anchors.fill: parent
        onPaint: {
            const ctx = getContext("2d"), r = live.r, c = live.census;
            ctx.reset();
            if (!c)
                return;
            const line = list => {
                ctx.beginPath();
                list.forEach((p, i) => i ? ctx.lineTo(p[0] * r, p[1] * r) : ctx.moveTo(p[0] * r, p[1] * r));
                ctx.stroke();
            };
            ctx.lineWidth = 1.5;
            ctx.strokeStyle = Kirigami.Theme.visitedLinkColor;
            c.trails.forEach(line);
            ctx.lineWidth = 2;
            ctx.strokeStyle = Kirigami.Theme.positiveTextColor;
            line(c.rope);
            live.dots.forEach(p => {
                    ctx.fillStyle = live.near(p) ? Kirigami.Theme.positiveTextColor : Kirigami.Theme.textColor;
                    ctx.fillRect(p[0] * r - 1.5, p[1] * r - 1.5, 3, 3);
                });
            const p = live.picked;
            if (p && live.reach > 0) {
                ctx.strokeStyle = Kirigami.Theme.highlightColor;
                ctx.beginPath();
                ctx.arc(p[0] * r, p[1] * r, live.reach * r, 0, 2 * Math.PI);
                ctx.stroke();
            }
        }
    }

    TapHandler {
        enabled: live.visible && live.reach > 0
        onTapped: point => {
            const at = [point.position.x / live.r, point.position.y / live.r];
            const d = p => Math.hypot(p[0] - at[0], p[1] - at[1]);
            const best = live.dots.reduce((a, p) => a === null || d(p) < d(a) ? p : a, null);
            SceneSwarmCensus.picked = best && d(best) * live.r < 16 ? best[2] : -1;
        }
    }
}
