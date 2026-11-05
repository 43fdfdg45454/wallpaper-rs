import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: knobs

    required property var frame
    readonly property var overlay: frame.overlay
    readonly property real knob: Kirigami.Units.gridUnit * 0.7
    readonly property var corner: overlay.kind === "line" ? [((overlay.spec.to ?? [0, 0])[0]) / 2, ((overlay.spec.to ?? [0, 0])[1]) / 2] : [overlay.box[0] / 2, overlay.box[1] / 2]
    readonly property real feather: overlay.mask?.feather ?? 0

    Rectangle {
        x: knobs.width / 2 + knobs.corner[0] * knobs.frame.stretch[0] - width / 2
        y: knobs.height / 2 + knobs.corner[1] * knobs.frame.stretch[1] - height / 2
        width: knobs.knob
        height: width
        color: Kirigami.Theme.highlightColor

        QQC2.ToolTip.text: App.tr("Drag to size the mask")
        QQC2.ToolTip.visible: sizeHover.hovered

        HoverHandler {
            id: sizeHover
        }

        DragHandler {
            target: null
            onCentroidChanged: {
                if (active)
                    knobs.overlay.sized(knobs.frame.local(parent, centroid.position));
            }
        }
    }

    Rectangle {
        x: knobs.width / 2 + (knobs.overlay.box[0] / 2 + knobs.feather) * knobs.frame.stretch[0]
        y: knobs.height / 2 - height / 2
        width: knobs.knob
        height: width
        radius: width / 2
        color: "transparent"
        border.color: Kirigami.Theme.highlightColor
        border.width: 2

        QQC2.ToolTip.text: App.tr("Soft edge %1 px: drag away from the shape to soften its edge", knobs.feather)
        QQC2.ToolTip.visible: featherHover.hovered

        HoverHandler {
            id: featherHover
        }

        DragHandler {
            target: null
            onCentroidChanged: {
                if (!active)
                    return;
                const at = knobs.frame.local(parent, centroid.position);
                knobs.overlay.put({
                        "feather": Math.max(0, Math.min(10000, Math.round(Math.abs(at[0]) - knobs.overlay.box[0] / 2)))
                    });
            }
        }
    }
}
