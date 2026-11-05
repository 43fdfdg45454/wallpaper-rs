import QtQuick
import io.github.wallpaper_rs.app

QtObject {
    id: check

    required property var page

    function read(pointer) {
        return JSON.parse(page.editor.value(pointer) || "null");
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
        Qt.createComponent("SceneSeedingCheck.qml").createObject(page, {
            "page": page
        }).exercise();
    }
}
