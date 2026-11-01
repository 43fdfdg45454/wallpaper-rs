import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Rectangle {
    id: knob

    required property var handles
    required property string modelData
    readonly property var head: handles.point(handles.picked.at)
    readonly property real angle: (handles.picked.angle + (modelData === "grow" ? 90 : 0)) * Math.PI / 180
    readonly property real out: modelData === "move" ? 0 : handles.reach

    x: head.x + Math.cos(angle) * out - width / 2
    y: head.y + Math.sin(angle) * out - height / 2
    width: Kirigami.Units.gridUnit * (modelData === "move" ? 1.1 : 0.8)
    height: width
    radius: modelData === "grow" ? 0 : width / 2
    color: modelData === "move" ? "transparent" : Kirigami.Theme.positiveTextColor
    border.color: Kirigami.Theme.positiveTextColor
    border.width: 2

    QQC2.ToolTip.visible: hover.hovered
    QQC2.ToolTip.text: knob.modelData === "move" ? App.tr("Move the bone") : knob.modelData === "turn" ? App.tr("Turn the bone") : App.tr("Scale the bone")

    HoverHandler {
        id: hover
    }

    DragHandler {
        target: null
        onActiveChanged: {
            if (active)
                knob.handles.grab(knob.mapToItem(knob.handles, centroid.position.x, centroid.position.y));
        }
        onCentroidChanged: {
            if (active)
                knob.handles.drag(knob.modelData, knob.mapToItem(knob.handles, centroid.position.x, centroid.position.y));
        }
    }
}
