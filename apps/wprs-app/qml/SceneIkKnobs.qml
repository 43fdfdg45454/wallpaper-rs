import QtQuick
import org.kde.kirigami as Kirigami

Item {
    id: knobs

    required property var host
    readonly property var chains: host.shown.ik ?? []
    readonly property var size: host.shown.size ?? [1, 1]
    readonly property real reach: Kirigami.Units.gridUnit * 2.5
    readonly property var aims: chains.map((c, i) => ({
                "index": i,
                "at": c.target,
                "follows": c.follows
            })).filter(t => t.follows === "point" && t.at)
    readonly property var ends: {
        const out = [];
        chains.forEach((c, i) => c.limits.forEach(l => [0, 1].forEach(side => out.push({
                        "index": i,
                        "limit": l,
                        "side": side
                    }))));
        return out;
    }

    function run(index, args) {
        host.page.run("ik", Object.assign({
            "layer": host.layerId,
            "index": index
        }, args));
    }

    Repeater {
        model: knobs.aims

        delegate: Rectangle {
            id: aim

            required property var modelData
            property point held: Qt.point(modelData.at[0] * knobs.width, modelData.at[1] * knobs.height)

            x: held.x - width / 2
            y: held.y - height / 2
            width: Kirigami.Units.gridUnit
            height: width
            radius: width / 2
            color: "transparent"
            border.color: Kirigami.Theme.neutralTextColor
            border.width: 3

            DragHandler {
                target: null
                onCentroidChanged: {
                    if (active)
                        aim.held = aim.mapToItem(knobs, centroid.position.x, centroid.position.y);
                }
                onActiveChanged: {
                    if (!active)
                        knobs.run(aim.modelData.index, {
                            "target": [Math.round(aim.held.x / knobs.width * knobs.size[0] * 10) / 10, Math.round(aim.held.y / knobs.height * knobs.size[1] * 10) / 10]
                        });
                }
            }
        }
    }

    Repeater {
        model: knobs.ends

        delegate: Rectangle {
            id: end

            required property var modelData
            readonly property var limit: modelData.limit
            property real turn: modelData.side === 0 ? limit.min : limit.max
            readonly property real angle: (limit.rest + turn) * Math.PI / 180

            x: limit.at[0] * knobs.width + knobs.reach * Math.cos(angle) - width / 2
            y: limit.at[1] * knobs.height + knobs.reach * Math.sin(angle) - height / 2
            width: Kirigami.Units.gridUnit * 0.7
            height: width
            radius: width / 2
            color: Kirigami.Theme.positiveTextColor

            DragHandler {
                target: null
                onCentroidChanged: {
                    if (!active)
                        return;
                    const p = end.mapToItem(knobs, centroid.position.x, centroid.position.y);
                    const deg = Math.atan2(p.y - end.limit.at[1] * knobs.height, p.x - end.limit.at[0] * knobs.width) * 180 / Math.PI - end.limit.rest;
                    const wrapped = Math.round(((deg + 540) % 360) - 180);
                    end.turn = end.modelData.side === 0 ? Math.min(wrapped, end.limit.max) : Math.max(wrapped, end.limit.min);
                }
                onActiveChanged: {
                    if (active)
                        return;
                    const both = end.modelData.side === 0 ? [end.turn, end.limit.max] : [end.limit.min, end.turn];
                    const limits = {};
                    limits[end.limit.bone] = both;
                    knobs.run(end.modelData.index, {
                        "limits": limits
                    });
                }
            }
        }
    }
}
