import QtQuick
import QtTest

TestEvent {
    id: events

    required property var page
    property var stage: null

    function made(name, props) {
        if (!stage)
            stage = Qt.createQmlObject("import QtQuick; Window { width: 640; height: 480; visible: true }", page);
        const item = Qt.createComponent(name).createObject(stage.contentItem, props);
        if (!item)
            throw new Error(name + " did not load");
        return item;
    }

    function finish() {
        if (stage)
            stage.destroy();
    }

    function center(item, within) {
        return item.mapToItem(within, item.width / 2, item.height / 2);
    }

    function drag(within, from, to) {
        mousePress(within, from.x, from.y, Qt.LeftButton, Qt.NoModifier, -1);
        for (let i = 1; i <= 8; i++)
            mouseMove(within, from.x + (to.x - from.x) * i / 8, from.y + (to.y - from.y) * i / 8, -1, Qt.LeftButton, Qt.NoModifier);
        mouseRelease(within, to.x, to.y, Qt.LeftButton, Qt.NoModifier, -1);
    }

    function find(item, test) {
        for (const child of item.children) {
            if (test(child))
                return child;
            const inner = find(child, test);
            if (inner)
                return inner;
        }
        return null;
    }

    function value(pointer) {
        return JSON.parse(page.editor.value(pointer) || "null");
    }

    function emitters(layer) {
        const at = layer + "/effects/0/params/emitters";
        page.run("set", {
            "pointer": at,
            "value": [{ "from": [0.25, 0.5], "angle": -90, "strength": 4 }]
        });
        const canvas = made("SceneEmitters.qml", {
            "page": page,
            "pointer": layer,
            "width": 400,
            "height": 200
        });
        const knob = find(canvas, c => c.modelData?.end === "from" && c.radius > 2);
        if (!knob)
            throw new Error("the emitter has no knob to drag");
        drag(canvas, center(knob, canvas), Qt.point(300, 100));
        const moved = value(at)[0].from;
        canvas.destroy();
        if (Math.abs(moved[0] - 0.75) > 0.02 || Math.abs(moved[1] - 0.5) > 0.02)
            throw new Error("dragging the emitter knob with the pointer did not move it: " + JSON.stringify(moved));
    }

    function bend(shown) {
        const knobs = made("SceneIkKnobs.qml", {
            "host": {
                "shown": shown,
                "page": page,
                "layerId": "puppet"
            },
            "width": 400,
            "height": 200
        });
        const knob = find(knobs, c => c.clockwise !== undefined && c.chain !== undefined);
        if (!knob)
            throw new Error("the chain has no bend knob");
        const at = center(knob, knobs);
        mouseClick(knobs, at.x, at.y, Qt.LeftButton, Qt.NoModifier, -1);
        knobs.destroy();
    }

    function depth() {
        const at = page.mesh.pointer("puppet") + "/depth";
        if (value(at) === null)
            page.run("set", {
                "pointer": at,
                "value": {
                    "scale": 1
                }
            });
        const panel = made("SceneRigDepth.qml", {
            "page": page,
            "layerId": "puppet",
            "editable": true,
            "width": 400
        });
        const box = find(panel, c => c.objectName === "sorted" && c.visible);
        if (!box)
            throw new Error("the depth panel has no switch to click");
        const spot = center(box, panel);
        mouseClick(panel, spot.x, spot.y, Qt.LeftButton, Qt.NoModifier, -1);
        panel.destroy();
    }
}
