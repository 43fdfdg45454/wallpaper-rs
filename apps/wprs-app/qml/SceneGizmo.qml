import QtQuick
import org.kde.kirigami as Kirigami

Item {
    id: gizmo

    required property var page
    required property int index
    required property real ratio
    readonly property var modelData: page.view.gizmos[index] ?? {
        "pointer": "",
        "offset": [0, 0],
        "position": [0, 0],
        "size": [0, 0],
        "scale": [1, 1],
        "rotation": 0,
        "locked": true
    }
    readonly property bool chosen: page.editor.selected === modelData.pointer
    readonly property bool movable: chosen && !modelData.locked
    readonly property var center: [modelData.offset[0] + modelData.position[0], modelData.offset[1] + modelData.position[1]]
    readonly property string base: modelData.pointer + "/transform"
    property var grabbed: null

    function moved(point) {
        const dx = (point.scenePosition.x - point.scenePressPosition.x) / ratio;
        const dy = (point.scenePosition.y - point.scenePressPosition.y) / ratio;
        const angle = -modelData.rotation * Math.PI / 180;
        return [dx, dy, dx * Math.cos(angle) - dy * Math.sin(angle), dx * Math.sin(angle) + dy * Math.cos(angle)];
    }

    function commit(field, value) {
        page.change(base + "/" + field, value.map(v => Math.round(v * 100) / 100), true);
    }

    x: center[0] * ratio - width / 2
    y: center[1] * ratio - height / 2
    width: Math.max(8, Math.abs(modelData.size[0] * modelData.scale[0]) * ratio)
    height: Math.max(8, Math.abs(modelData.size[1] * modelData.scale[1]) * ratio)
    rotation: modelData.rotation
    z: chosen ? 2 : 1

    Rectangle {
        anchors.fill: parent
        color: "transparent"
        border.color: gizmo.chosen ? Kirigami.Theme.highlightColor : Kirigami.Theme.disabledTextColor
        border.width: gizmo.chosen ? 2 : 1
        opacity: gizmo.chosen ? 1 : 0.5

        HoverHandler {
            id: hover
        }

        Rectangle {
            anchors.fill: parent
            color: "transparent"
            border.color: Kirigami.Theme.highlightColor
            opacity: hover.hovered && !gizmo.chosen ? 0.5 : 0
        }
    }

    TapHandler {
        onTapped: gizmo.page.editor.select(gizmo.modelData.pointer)
    }

    DragHandler {
        target: null
        enabled: !gizmo.modelData.locked
        acceptedModifiers: Qt.NoModifier
        onActiveChanged: {
            gizmo.page.moving = active;
            if (active && !gizmo.chosen)
                gizmo.page.editor.select(gizmo.modelData.pointer);
            gizmo.grabbed = gizmo.modelData.position;
        }
        onCentroidChanged: {
            const delta = gizmo.moved(centroid);
            if (active && gizmo.grabbed)
                gizmo.commit("position", [gizmo.grabbed[0] + delta[0], gizmo.grabbed[1] + delta[1]]);
        }
    }

    Rectangle {
        id: turn

        visible: gizmo.movable
        x: gizmo.width / 2 - width / 2
        y: -Kirigami.Units.gridUnit * 1.5
        width: Kirigami.Units.gridUnit * 0.8
        height: width
        radius: width / 2
        color: Kirigami.Theme.highlightColor

        DragHandler {
            target: null
            onCentroidChanged: {
                if (!active)
                    return;
                const point = turn.mapToItem(gizmo.parent, centroid.position.x, centroid.position.y);
                const dx = point.x - gizmo.center[0] * gizmo.ratio, dy = point.y - gizmo.center[1] * gizmo.ratio;
                gizmo.page.change(gizmo.base + "/rotation", Math.round(Math.atan2(dx, -dy) * 1800 / Math.PI) / 10, true);
            }
        }
    }

    Rectangle {
        id: grow

        visible: gizmo.movable
        x: gizmo.width - width / 2
        y: gizmo.height - height / 2
        width: Kirigami.Units.gridUnit * 0.8
        height: width
        color: Kirigami.Theme.highlightColor

        DragHandler {
            target: null
            onActiveChanged: gizmo.grabbed = gizmo.modelData.scale
            onCentroidChanged: {
                if (!active || !gizmo.grabbed)
                    return;
                const delta = gizmo.moved(centroid);
                const w = Math.max(1, gizmo.modelData.size[0] * gizmo.grabbed[0]), h = Math.max(1, gizmo.modelData.size[1] * gizmo.grabbed[1]);
                gizmo.commit("scale", [gizmo.grabbed[0] * (1 + 2 * delta[2] / w), gizmo.grabbed[1] * (1 + 2 * delta[3] / h)]);
            }
        }
    }
}
