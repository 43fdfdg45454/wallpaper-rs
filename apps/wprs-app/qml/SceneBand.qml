import QtQuick
import org.kde.kirigami as Kirigami

Item {
    id: band

    required property var page
    required property Item screen
    required property real ratio
    property rect area: Qt.rect(0, 0, 0, 0)

    function pick() {
        const at = mapToItem(screen, area.x, area.y), left = at.x / ratio, top = at.y / ratio;
        const right = left + area.width / ratio, bottom = top + area.height / ratio;
        const inside = gizmo => {
            const cx = gizmo.offset[0] + gizmo.position[0], cy = gizmo.offset[1] + gizmo.position[1];
            const w = Math.abs(gizmo.size[0] * gizmo.scale[0]) / 2, h = Math.abs(gizmo.size[1] * gizmo.scale[1]) / 2;
            return cx - w < right && cx + w > left && cy - h < bottom && cy + h > top;
        };
        const pointers = (page.view.gizmos ?? []).filter(inside).map(gizmo => gizmo.pointer);
        const layers = page.view.layers ?? [];
        page.picked = pointers.map(pointer => layers.find(layer => layer.pointer === pointer)?.id).filter(id => id);
        if (pointers.length)
            page.editor.select(pointers[pointers.length - 1]);
    }

    DragHandler {
        id: drag

        target: null
        acceptedButtons: Qt.LeftButton
        onActiveChanged: {
            if (!active && band.area.width * band.area.height > 0)
                band.pick();
            if (!active)
                band.area = Qt.rect(0, 0, 0, 0);
        }
        onCentroidChanged: {
            if (!active)
                return;
            const a = centroid.pressPosition, b = centroid.position;
            band.area = Qt.rect(Math.min(a.x, b.x), Math.min(a.y, b.y), Math.abs(b.x - a.x), Math.abs(b.y - a.y));
        }
    }

    Rectangle {
        parent: band.parent
        x: band.area.x
        y: band.area.y
        z: 10
        width: band.area.width
        height: band.area.height
        visible: drag.active
        color: Qt.rgba(Kirigami.Theme.highlightColor.r, Kirigami.Theme.highlightColor.g, Kirigami.Theme.highlightColor.b, 0.2)
        border.color: Kirigami.Theme.highlightColor
    }
}
