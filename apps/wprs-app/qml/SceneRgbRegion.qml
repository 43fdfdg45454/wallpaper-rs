import QtQuick
import org.kde.kirigami as Kirigami

Item {
    id: region

    required property var page
    required property real ratio
    readonly property var gizmo: (page.view.gizmos ?? []).find(g => g.pointer === page.editor.selected) ?? null
    readonly property var entry: gizmo?.kind === "adjustment" ? JSON.parse(page.editor.value(gizmo.pointer) || "null") : null
    readonly property bool shown: entry?.rgb === true && Array.isArray(entry?.size) && !gizmo.locked
    readonly property var origin: entry?.transform?.origin ?? [0.5, 0.5]
    property var grabbed: null

    function resize(handle, centroid, signs) {
        if (!handle.active || !grabbed)
            return;
        const dx = (centroid.scenePosition.x - centroid.scenePressPosition.x) / ratio;
        const dy = (centroid.scenePosition.y - centroid.scenePressPosition.y) / ratio;
        const size = [Math.max(1, Math.round(grabbed[0] + signs[0] * dx)), Math.max(1, Math.round(grabbed[1] + signs[1] * dy))];
        page.run("rgb", {
            "layer": gizmo.id,
            "size": size
        }, true);
    }

    visible: shown

    Rectangle {
        id: box

        x: region.shown ? (region.gizmo.offset[0] + region.gizmo.position[0] - region.origin[0] * region.entry.size[0]) * region.ratio : 0
        y: region.shown ? (region.gizmo.offset[1] + region.gizmo.position[1] - region.origin[1] * region.entry.size[1]) * region.ratio : 0
        width: region.shown ? region.entry.size[0] * region.ratio : 0
        height: region.shown ? region.entry.size[1] * region.ratio : 0
        color: "transparent"
        border.color: Kirigami.Theme.positiveTextColor
        border.width: 2

        Repeater {
            model: [[0, 0, -1, -1], [1, 0, 1, -1], [0, 1, -1, 1], [1, 1, 1, 1]]

            delegate: Rectangle {
                id: corner

                required property var modelData

                x: modelData[0] * box.width - width / 2
                y: modelData[1] * box.height - height / 2
                width: Kirigami.Units.gridUnit * 0.8
                height: width
                radius: width / 4
                color: Kirigami.Theme.positiveTextColor

                DragHandler {
                    id: pull

                    target: null
                    onActiveChanged: region.grabbed = active ? region.entry.size : null
                    onCentroidChanged: region.resize(pull, centroid, [corner.modelData[2] * 2, corner.modelData[3] * 2])
                }
            }
        }
    }
}
