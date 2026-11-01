import QtQuick
import org.kde.kirigami as Kirigami

Item {
    id: limbs

    required property var host
    readonly property var mesh: host.mesh
    readonly property var page: host.gizmo.page
    readonly property string layerId: host.layerId
    readonly property string mode: mesh.rigging === layerId ? mesh.mode : ""
    property var shown: ({
            "ik": [],
            "bodies": [],
            "size": [1, 1]
        })
    property var rims: []
    property var pins: []

    function load() {
        if (!visible)
            return;
        shown = mesh.ask(layerId, {
            "handles": true
        }) ?? {
            "ik": [],
            "bodies": [],
            "size": [1, 1]
        };
        const base = mesh.pointer(layerId) + "/parts/" + mesh.part.replace(/~/g, "~0").replace(/\//g, "~1");
        const over = mesh.part !== "" ? JSON.parse(page.editor.value(base + "/clip_to") || "null") : null;
        const all = mesh.part !== "" ? mesh.ask(layerId, {
            "outline": true
        }) ?? {} : {};
        rims = [[mesh.part, all[mesh.part] ?? []], [over, all[over] ?? []]];
        pins = attached();
        drawing.requestPaint();
    }

    function attached() {
        const out = [], g = host.gizmo, spots = host.list.concat(host.rig.points ?? []);
        for (const other of page.view.gizmos ?? []) {
            const a = JSON.parse(page.editor.value(other.pointer + "/attach") || "null");
            const spot = a && a.layer === layerId ? spots.find(b => b.name === (a.bone ?? a.point)) : null;
            if (!spot || !spot.at)
                continue;
            const c = mapFromItem(g.parent, (other.offset[0] + other.position[0]) * g.ratio, (other.offset[1] + other.position[1]) * g.ratio);
            out.push([spot.at, [c.x / width, c.y / height]]);
        }
        return out;
    }

    function nearest(pos, most) {
        const far = b => Math.hypot(b.at[0] * width - pos.x, b.at[1] * height - pos.y);
        const list = host.list.filter(b => b.at && far(b) < most).sort((a, b) => far(a) - far(b));
        return list.length > 0 ? list[0].name : "";
    }

    function tapped(name) {
        if (mode === "chain" && mesh.root === "")
            mesh.root = name;
        else if (mode === "chain") {
            page.run("ik", {
                "layer": layerId,
                "root": mesh.root,
                "tip": name
            });
            mesh.root = "";
        } else if (mode === "bodies")
            page.run("physics", {
                "layer": layerId,
                "bones": [name],
                "remove": shown.bodies.some(b => b.bone === name)
            });
        else if (mode === "attach" && mesh.attaching !== "")
            pin(mesh.attaching, Qt.point(0, 0), name);
        else
            mesh.bone = name;
    }

    function pin(id, pos, bone) {
        const name = bone || nearest(pos, Kirigami.Units.gridUnit);
        page.run("attach", Object.assign({
            "layer": id,
            "to": layerId,
            "inherit": mesh.inherit
        }, name !== "" ? {
            "bone": name
        } : {
            "point": host.at(pos)
        }));
    }

    function rope(points) {
        const n = Math.min(9, points.length);
        const picked = Array.from({
            "length": n
        }, (_, i) => points[Math.round(i * (points.length - 1) / (n - 1))]);
        const taken = host.list.map(b => b.name);
        let name = "rope";
        for (let i = 2; taken.includes(name + "_1"); i++)
            name = "rope_" + i;
        const first = Qt.point(picked[0][0] * width, picked[0][1] * height);
        page.run("physics", {
            "layer": layerId,
            "bones": [name],
            "rope": picked,
            "parent": mesh.bone || nearest(first, Infinity) || null
        });
    }

    onVisibleChanged: load()
    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            limbs.load();
        }

        target: limbs.page.editor
    }

    Connections {
        function onPartChanged() {
            limbs.load();
        }

        target: limbs.mesh
    }

    SceneLimbLines {
        id: drawing

        anchors.fill: parent
        owner: limbs
    }

    TapHandler {
        enabled: limbs.mode === "attach" && limbs.mesh.attaching !== ""
        onTapped: (point, button) => {
            if (limbs.nearest(point.position, Kirigami.Units.gridUnit) === "")
                limbs.pin(limbs.mesh.attaching, point.position, "");
        }
    }

    DragHandler {
        property var points: []

        target: null
        enabled: limbs.mode === "rope"
        onActiveChanged: {
            if (!active && points.length >= 2)
                limbs.rope(points);
            points = [];
        }
        onCentroidChanged: {
            if (active && points.length < 256)
                points = points.concat([limbs.host.at(centroid.position)]);
        }
    }

    DropArea {
        anchors.fill: parent
        keys: ["wprs-layer"]
        onDropped: drop => limbs.pin(drop.source.layerId, Qt.point(drop.x, drop.y), "")
    }

    SceneIkKnobs {
        anchors.fill: parent
        host: limbs
    }
}
