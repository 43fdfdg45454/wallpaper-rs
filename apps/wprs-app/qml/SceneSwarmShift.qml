import QtQuick
import io.github.wallpaper_rs.app

Item {
    id: shift

    required property var swarm
    readonly property var info: SceneSwarmGlow.info?.system === swarm.system ? SceneSwarmGlow.info : null
    readonly property var census: SceneSwarmCensus.census?.system === swarm.system ? SceneSwarmCensus.census : null

    visible: SceneSwarmGlow.shown && !!info?.field && census !== null

    Repeater {
        model: shift.visible ? shift.census.particles : []

        delegate: Image {
            required property var modelData

            readonly property real side: Math.max(4, shift.info.size * shift.swarm.ratio)

            x: modelData[0] * shift.swarm.ratio - side / 2
            y: modelData[1] * shift.swarm.ratio - side / 2
            width: side
            height: side
            opacity: 0.35 + 0.5 * Math.min(1, shift.info.refract?.amount ?? 0)
            source: shift.info.field
        }
    }
}
