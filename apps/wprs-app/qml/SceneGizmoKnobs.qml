import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: knobs

    required property var gizmo
    readonly property real knob: Kirigami.Units.gridUnit * 0.8
    property var grabbed: null

    function leaned(delta) {
        const span = Math.max(1, Math.max(Math.abs(gizmo.modelData.size[0] * gizmo.modelData.scale[0]), Math.abs(gizmo.modelData.size[1] * gizmo.modelData.scale[1])));
        const angle = v => Math.max(-89, Math.min(89, Math.round(v * 10) / 10));
        return [angle(grabbed[0] - delta[3] * 180 / span), angle(grabbed[1] + delta[2] * 180 / span)];
    }

    visible: gizmo.movable

    Rectangle {
        id: turn

        x: knobs.width / 2 - width / 2
        y: -Kirigami.Units.gridUnit * 1.5
        width: knobs.knob
        height: width
        radius: width / 2
        color: Kirigami.Theme.highlightColor

        DragHandler {
            target: null
            onCentroidChanged: {
                if (!active)
                    return;
                const point = turn.mapToItem(knobs.gizmo.parent, centroid.position.x, centroid.position.y);
                const dx = point.x - knobs.gizmo.center[0] * knobs.gizmo.ratio, dy = point.y - knobs.gizmo.center[1] * knobs.gizmo.ratio;
                knobs.gizmo.commit("rotation", Math.atan2(dx, -dy) * 180 / Math.PI);
            }
        }
    }

    Rectangle {
        x: knobs.width - width / 2
        y: knobs.height - height / 2
        width: knobs.knob
        height: width
        color: Kirigami.Theme.highlightColor

        DragHandler {
            target: null
            onActiveChanged: knobs.grabbed = knobs.gizmo.modelData.scale
            onCentroidChanged: {
                if (!active || !knobs.grabbed)
                    return;
                const delta = knobs.gizmo.moved(centroid);
                const w = Math.max(1, knobs.gizmo.modelData.size[0] * knobs.grabbed[0]), h = Math.max(1, knobs.gizmo.modelData.size[1] * knobs.grabbed[1]);
                knobs.gizmo.commit("scale", [knobs.grabbed[0] * (1 + 2 * delta[2] / w), knobs.grabbed[1] * (1 + 2 * delta[3] / h)]);
            }
        }
    }

    Rectangle {
        x: -width / 2
        y: knobs.height / 2 - height / 2
        width: knobs.knob
        height: width
        radius: width / 2
        color: "transparent"
        border.color: Kirigami.Theme.highlightColor
        border.width: 2

        QQC2.ToolTip.text: App.tr("Tilt %1°, %2°: drag up or down to tilt around the width, sideways around the height", knobs.gizmo.modelData.tilt[0], knobs.gizmo.modelData.tilt[1])
        QQC2.ToolTip.visible: leanHover.hovered

        HoverHandler {
            id: leanHover
        }

        DragHandler {
            target: null
            onActiveChanged: knobs.grabbed = knobs.gizmo.modelData.tilt
            onCentroidChanged: {
                if (active && knobs.grabbed)
                    knobs.gizmo.commit("tilt", knobs.leaned(knobs.gizmo.moved(centroid)));
            }
        }
    }

    Rectangle {
        x: knobs.width / 2 - width / 2
        y: knobs.height + Kirigami.Units.gridUnit * 0.7
        width: knobs.knob
        height: width
        rotation: 45
        color: "transparent"
        border.color: Kirigami.Theme.highlightColor
        border.width: 2

        QQC2.ToolTip.text: App.tr("Depth %1 px: drag up to bring the layer towards the viewer, down to push it back", knobs.gizmo.modelData.z)
        QQC2.ToolTip.visible: liftHover.hovered

        HoverHandler {
            id: liftHover
        }

        DragHandler {
            target: null
            onActiveChanged: knobs.grabbed = [knobs.gizmo.modelData.z]
            onCentroidChanged: {
                if (active && knobs.grabbed)
                    knobs.gizmo.commit("z", Math.max(-100000, Math.min(100000, knobs.grabbed[0] - knobs.gizmo.moved(centroid)[1] * 2)));
            }
        }
    }
}
