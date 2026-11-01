import QtQuick
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: focal

    required property var swarm
    readonly property real length: swarm.info?.perspective?.focal ?? 0
    readonly property var middle: [width / swarm.ratio / 2, height / swarm.ratio / 2]

    visible: length > 0

    Rectangle {
        x: parent.width / 2 - width / 2
        y: parent.height / 2 - height / 2
        width: focal.length / 10 * focal.swarm.ratio * 2
        height: width
        radius: width / 2
        color: "transparent"
        border.color: Kirigami.Theme.neutralTextColor
        border.width: 1
    }

    SceneKnob {
        at: [focal.middle[0] + focal.length / 10, focal.middle[1]]
        ratio: focal.swarm.ratio
        label: App.tr("Focal length %1 px", Math.round(focal.length))
        tip: App.tr("Drag to change how strongly depth shrinks particles")
        onMoved: (to, done) => focal.swarm.edit("perspective", {
                "focal": Math.max(1, Math.round(Math.hypot(to[0] - focal.middle[0], to[1] - focal.middle[1]) * 10))
            })
    }
}
