import QtQuick
import io.github.wallpaper_rs.app

QtObject {
    required property var page

    function read(pointer) {
        return JSON.parse(page.editor.value(pointer) || "null");
    }

    function exercise() {
        const steps = page.view.history.done.length;
        const id = read("/scene/layers/0/id");
        page.run("effects", {
                "add": {
                    "layer": id,
                    "effect": "vignette"
                }
            });
        const index = (read("/scene/layers/0/effects") ?? []).length - 1;
        const use = "/scene/layers/0/effects/" + index;
        page.editor.select(use);
        const panel = Qt.createComponent("SceneEffectMask.qml").createObject(page, {
                "page": page,
                "pointer": use
            });
        const overlay = Qt.createComponent("SceneMaskOverlay.qml").createObject(page, {
                "page": page,
                "ratio": 1
            });
        panel.put({
                "shape": panel.shaped("rect")
            });
        if (panel.source !== "shape" || panel.kind !== "rect" || overlay.kind !== "rect")
            throw new Error("mask: the panel did not give the effect a shape: " + page.editor.error);
        overlay.sized([60, -25]);
        overlay.put({
                "feather": 12
            });
        const mask = read(use + "/mask");
        if (JSON.stringify(mask.shape.rect.size) !== "[120,50]" || mask.feather !== 12)
            throw new Error("mask: the handles did not size or soften the shape: " + JSON.stringify(mask));
        const before = page.view.history.done.length;
        page.editor.hold(true);
        overlay.put({
                "feather": 20
            });
        const until = Date.now() + 700;
        while (Date.now() < until) {}
        overlay.put({
                "feather": 30
            });
        page.editor.hold(false);
        if (page.view.history.done.length !== before + 1 || read(use + "/mask/feather") !== 30)
            throw new Error("mask: a held drag of the soft edge that paused took " + (page.view.history.done.length - before) + " steps");
        panel.put({
                "shape": panel.shaped("star")
            });
        overlay.sized([40, 10]);
        if (read(use + "/mask/shape/star/size") !== 80)
            throw new Error("mask: the star was not sized from its handle");
        panel.put({
                "off": true
            });
        if (read(use + "/mask") !== null || overlay.visible)
            throw new Error("mask: the mask was not taken off");
        panel.destroy();
        overlay.destroy();
        while (page.view.history.done.length > steps)
            page.run("undo");
    }
}
