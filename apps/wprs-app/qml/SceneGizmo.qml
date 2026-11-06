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
        "tilt": [0, 0],
        "z": 0,
        "corners": null,
        "locked": true
    }
    readonly property bool chosen: page.editor.selected === modelData.pointer
    readonly property bool movable: chosen && !modelData.locked
    readonly property bool brushing: ["/effects/", "/lighting"].some(part => (page.brush?.pointer ?? "").startsWith(modelData.pointer + part))
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
        const rounded = Array.isArray(value) ? value.map(v => Math.round(v * 100) / 100) : Math.round(value * 100) / 100;
        page.run("layer", {
                "transform": {
                    "layer": modelData.id,
                    [field]: rounded,
                    "at": page.editor.time,
                    "replace": page.overwrite
                }
            }, true);
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
        opacity: (gizmo.chosen ? 1 : 0.5) * (gizmo.modelData.corners ? 0.4 : 1)

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

    SceneMapHeat {
        id: heat

        anchors.fill: parent
        page: gizmo.page
        visible: gizmo.brushing && gizmo.page.brush?.action === "paint_map"
    }

    DragHandler {
        property var points: []

        target: null
        enabled: gizmo.brushing
        onActiveChanged: {
            if (!active && points.length > 0)
                gizmo.page.run(gizmo.page.brush.action ?? "paint_mask", heat.stroke(gizmo.page.brush, points));
            points = [];
        }
        onCentroidChanged: {
            if (active)
                points = points.concat([[centroid.position.x / gizmo.width, centroid.position.y / gizmo.height]]);
        }
    }

    DragHandler {
        target: null
        enabled: !gizmo.modelData.locked && !gizmo.brushing && gizmo.page.mesh.rigging !== gizmo.modelData.id && !gizmo.page.mesh.posing(gizmo.modelData.id)
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

    SceneGizmoKnobs {
        anchors.fill: parent
        gizmo: gizmo
    }

    SceneGizmoLean {
        gizmo: gizmo
    }

    SceneEmitters {
        anchors.fill: parent
        z: 3
        visible: gizmo.chosen
        page: gizmo.page
        pointer: gizmo.chosen ? gizmo.modelData.pointer : ""
    }

    SceneBones {
        gizmo: gizmo
        visible: gizmo.page.mesh.rigging === gizmo.modelData.id || gizmo.page.mesh.posing(gizmo.modelData.id)
    }
}
