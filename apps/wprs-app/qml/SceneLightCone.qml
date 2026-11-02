import QtQuick
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: cone

    required property var gizmo
    readonly property var light: gizmo.light
    readonly property real ratio: gizmo.ratio
    readonly property bool casting: light.type === "spot" && !!light.texture
    property var info: null

    function load() {
        info = casting ? JSON.parse(gizmo.owner.page.editor.rig(light.id, JSON.stringify({
            "projection": {
                "layer": light.id,
                "time": gizmo.owner.page.editor.time
            }
        })) || "null") : null;
    }

    onLightChanged: load()
    Component.onCompleted: load()

    Rectangle {
        visible: cone.gizmo.chosen && !!cone.light.volumetric
        x: (cone.gizmo.at[0] - cone.light.radius) * cone.ratio
        y: (cone.gizmo.at[1] - cone.light.radius) * cone.ratio
        width: cone.light.radius * 2 * cone.ratio
        height: width
        radius: width / 2
        border.width: 0
        color: Qt.alpha(Kirigami.Theme.neutralTextColor, 0.08 + 0.2 * (cone.light.volumetric?.density ?? 0.2))
    }

    Image {
        visible: cone.gizmo.chosen && !!cone.info?.matrix && !!cone.info?.image
        width: 1
        height: 1
        opacity: 0.55
        source: cone.info?.image ?? ""
        smooth: true
        transform: [
            Matrix4x4 {
                matrix: {
                    const m = cone.info?.matrix ?? [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1];
                    return Qt.matrix4x4(m[0], m[1], m[2], m[3], m[4], m[5], m[6], m[7], m[8], m[9], m[10], m[11], m[12], m[13], m[14], m[15]);
                }
            },
            Scale {
                xScale: cone.ratio
                yScale: cone.ratio
            }
        ]
    }

    SceneLightLines {
        anchors.fill: parent
        visible: cone.gizmo.chosen && !!cone.info?.quad
        ratio: cone.ratio
        dashed: true
        segments: {
            const q = cone.info?.quad ?? [];
            return q.length === 4 ? [[q[0], q[1]], [q[1], q[2]], [q[2], q[3]], [q[3], q[0]]] : [];
        }
    }
}
