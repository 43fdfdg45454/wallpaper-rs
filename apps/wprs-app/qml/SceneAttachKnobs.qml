import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: knobs

    required property var host
    readonly property var size: host.shown.size ?? [1, 1]

    Repeater {
        model: knobs.visible ? knobs.host.pins : []

        delegate: Rectangle {
            id: knob

            required property var modelData
            property point start

            x: modelData.to[0] * knobs.width - width / 2
            y: modelData.to[1] * knobs.height - height / 2
            width: Kirigami.Units.gridUnit * 0.9
            height: width
            radius: width / 2
            color: "transparent"
            border.color: Kirigami.Theme.highlightColor
            border.width: 3
            QQC2.ToolTip.visible: hover.hovered
            QQC2.ToolTip.text: App.tr("Drag to move %1 along what it follows", modelData.id)

            HoverHandler {
                id: hover
            }

            DragHandler {
                onActiveChanged: {
                    if (active) {
                        knob.start = Qt.point(knob.x, knob.y);
                        return;
                    }
                    const dx = Math.round((knob.x - knob.start.x) / knobs.width * knobs.size[0] * 10) / 10;
                    const dy = Math.round((knob.y - knob.start.y) / knobs.height * knobs.size[1] * 10) / 10;
                    if (dx !== 0 || dy !== 0)
                        knobs.host.page.run("attach", {
                            "layer": knob.modelData.id,
                            "nudge": [dx, dy]
                        });
                }
            }
        }
    }
}
