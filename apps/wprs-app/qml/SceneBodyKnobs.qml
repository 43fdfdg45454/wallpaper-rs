import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: knobs

    required property var host
    readonly property var bodies: visible ? host.shown.bodies ?? [] : []
    readonly property real unit: Kirigami.Units.gridUnit * 0.6
    readonly property real reach: Kirigami.Units.gridUnit * 2.5
    readonly property var ends: {
        const out = [];
        bodies.filter(b => b.rotation).forEach(b => [0, 1].forEach(side => out.push({
                    "body": b,
                    "side": side
                })));
        return out;
    }

    function run(bone, args) {
        host.page.run("physics", Object.assign({
            "layer": host.layerId,
            "bones": [bone]
        }, args));
    }

    SceneRepeater {
        id: listed2

        rows: knobs.bodies

        delegate: Rectangle {
            id: pull

            required property int index
            readonly property var modelData: listed2.rows[index]
            property var pulled: null
            readonly property real gravity: pulled ?? modelData.gravity

            x: modelData.at[0] * knobs.width - width / 2
            y: modelData.at[1] * knobs.height + gravity * knobs.unit - height / 2
            width: Kirigami.Units.gridUnit * 0.7
            height: width
            color: Kirigami.Theme.negativeTextColor
            QQC2.ToolTip.visible: hover.hovered
            QQC2.ToolTip.text: App.tr("Drag up or down to set the gravity of %1", modelData.bone)

            HoverHandler {
                id: hover
            }

            DragHandler {
                target: null
                onCentroidChanged: {
                    if (!active)
                        return;
                    const p = pull.mapToItem(knobs, centroid.position.x, centroid.position.y);
                    const g = (p.y - pull.modelData.at[1] * knobs.height) / knobs.unit;
                    pull.pulled = Math.max(-10, Math.min(10, Math.round(g * 10) / 10));
                }
                onActiveChanged: {
                    if (active)
                        return;
                    knobs.run(pull.modelData.bone, {
                        "gravity": pull.gravity
                    });
                    pull.pulled = null;
                }
            }
        }
    }

    SceneRepeater {
        id: listed

        rows: knobs.ends

        delegate: Rectangle {
            id: end

            required property int index
            readonly property var modelData: listed.rows[index]
            readonly property var body: modelData.body
            property var turned: null
            readonly property real turn: turned ?? body.rotation[modelData.side]
            readonly property real angle: (body.rest + turn) * Math.PI / 180

            x: body.at[0] * knobs.width + knobs.reach * Math.cos(angle) - width / 2
            y: body.at[1] * knobs.height + knobs.reach * Math.sin(angle) - height / 2
            width: Kirigami.Units.gridUnit * 0.7
            height: width
            radius: width / 2
            color: Kirigami.Theme.neutralTextColor

            DragHandler {
                target: null
                onCentroidChanged: {
                    if (!active)
                        return;
                    const p = end.mapToItem(knobs, centroid.position.x, centroid.position.y);
                    const deg = Math.atan2(p.y - end.body.at[1] * knobs.height, p.x - end.body.at[0] * knobs.width) * 180 / Math.PI - end.body.rest;
                    const wrapped = Math.round(((deg + 540) % 360) - 180);
                    end.turned = end.modelData.side === 0 ? Math.min(wrapped, end.body.rotation[1]) : Math.max(wrapped, end.body.rotation[0]);
                }
                onActiveChanged: {
                    if (active)
                        return;
                    knobs.run(end.body.bone, {
                        "rotation": end.modelData.side === 0 ? [end.turn, end.body.rotation[1]] : [end.body.rotation[0], end.turn]
                    });
                    end.turned = null;
                }
            }
        }
    }
}
