import QtQuick
import io.github.wallpaper_rs.app

QtObject {
    id: check

    required property var page

    function read(pointer) {
        return JSON.parse(page.editor.value(pointer) || "null");
    }

    function dragged() {
        const steps = page.view.history.done.length;
        const look = Qt.createComponent("SceneCanvasLook.qml").createObject(page, {
            "page": page
        });
        look.slid("bloom", "strength", 1.5, 1);
        if (read("/scene/canvas/bloom/strength") !== 1.5)
            throw new Error("shaping: the glow did not change while its slider was dragged");
        for (const value of [2.5, 3.5, 4])
            look.slid("bloom", "strength", value, 1);
        if (read("/scene/canvas/bloom/strength") !== 4 || page.view.history.done.length !== steps + 1)
            throw new Error("shaping: one drag of the glow took other than one undo step");
        page.run("undo");
        const before = page.view.history.done.length;
        page.run("set", {
            "pointer": "/scene/canvas/bloom",
            "value": {}
        });
        const glowing = page.view.history.done.length;
        look.slid("bloom", "radius", 16, 16);
        look.slid("parallax", "amount", 1, 1);
        if (page.view.history.done.length !== glowing || read("/scene/canvas/bloom/radius") !== null)
            throw new Error("shaping: a click on a canvas slider that did not move it took an undo step");
        if (glowing !== before)
            page.run("undo");
        look.destroy();
    }

    function exercise() {
        const id = read("/scene/layers/0/id");
        const steps = page.view.history.done.length;
        const kids = (read("/scene/particles/flakes/children") ?? []).length;
        page.run("effects", {
            "add": {
                "layer": id,
                "effect": "vignette",
                "params": {
                    "intensity": 0.5
                }
            }
        });
        page.run("layer", {
            "blend": {
                "layer": id,
                "blend": "screen"
            }
        });
        page.run("canvas", {
            "bloom": {
                "strength": 2
            }
        });
        page.run("canvas", {
            "parallax": {
                "amount": 1.5
            }
        });
        page.run("particles", {
            "create": {
                "system": "flakes_child_1",
                "parent": "flakes",
                "on": "death"
            }
        });
        page.run("manifest", {
            "display": {
                "values": {
                    "span": "all-outputs"
                }
            }
        });
        if (page.editor.error !== "")
            throw new Error("shaping: " + page.editor.error);
        const effects = read("/scene/layers/0/effects") ?? [];
        if (effects[effects.length - 1]?.params?.intensity !== 0.5 || read("/scene/layers/0/blend") !== "screen")
            throw new Error("shaping: the effect or the blend mode of the layer was not set");
        if (read("/scene/canvas/bloom/strength") !== 2 || read("/scene/canvas/parallax/amount") !== 1.5)
            throw new Error("shaping: the canvas does not glow or move in depth");
        if ((read("/scene/particles/flakes/children") ?? []).length !== kids + 1 || read("/manifest/display/span") !== "all-outputs")
            throw new Error("shaping: no child system was created or the scene does not span the screens");
        if (page.view.history.done.length !== steps + 6)
            throw new Error("shaping: an authoring action did not take exactly one undo step");
        for (let k = 0; k < 6; k++)
            page.run("undo");
        dragged();
        Qt.createComponent("SceneMaskCheck.qml").createObject(page, {
            "page": page
        }).exercise();
        Qt.createComponent("SceneGizmoCheck.qml").createObject(page, {
            "page": page
        }).exercise();
        Qt.createComponent("SceneSeedingCheck.qml").createObject(page, {
            "page": page
        }).exercise();
    }
}
