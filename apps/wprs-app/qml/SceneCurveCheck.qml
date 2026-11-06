import QtQuick
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

QtObject {
    required property var page

    function read(pointer) {
        return JSON.parse(page.editor.value(pointer) || "null");
    }

    function bend(pointer) {
        const keys = read(pointer + "/keys");
        const timeline = {
            "page": page,
            "tracks": page.view.tracks,
            "chosen": {
                "pointer": pointer,
                "index": 0
            },
            "picked": keys[0]
        };
        const curve = Qt.createComponent("SceneKeyCurve.qml").createObject(page, {
                "timeline": timeline
            });
        curve.parent = QQC2.Overlay.overlay;
        curve.open();
        curve.refit();
        if (!curve.shaped)
            throw new Error("curve: the keys of " + pointer + " take no handles");
        const steps = page.view.history.done.length;
        curve.press("out");
        curve.grab(40, 30);
        curve.grab(60, 20);
        curve.commit();
        const out = read(pointer + "/keys/0/2/out");
        if (page.view.history.done.length !== steps + 1 || !Array.isArray(out))
            throw new Error("curve: one drag of a handle took " + (page.view.history.done.length - steps) + " steps: " + page.editor.error);
        curve.close();
        curve.destroy();
    }

    function slide() {
        const slider = Qt.createComponent("SceneSlider.qml").createObject(page);
        page.run("canvas", {
            "bloom": {}
        });
        const steps = page.view.history.done.length;
        for (const strengths of [[2, 3], [4]]) {
            slider.hold(true);
            for (const strength of strengths)
                page.run("canvas", {
                    "bloom": {
                        "strength": strength
                    }
                }, true);
            slider.hold(false);
        }
        if (page.view.history.done.length !== steps + 2)
            throw new Error("slider: two presses took " + (page.view.history.done.length - steps) + " steps");
        slider.destroy();
    }

    function exercise() {
        const steps = page.view.history.done.length;
        slide();
        const pointer = "/scene/layers/0/transform/position";
        const keys = [[pointer, 0, [0, 0]], [pointer, 2, [200, 100]], ["/scene/layers/0/opacity", 0, 0], ["/scene/layers/0/opacity", 2, 1]];
        for (const key of keys)
            page.run("keyframe", {
                    "pointer": key[0],
                    "time": key[1],
                    "value": key[2]
                });
        bend(pointer);
        bend("/scene/layers/0/opacity");
        while (page.view.history.done.length > steps)
            page.run("undo");
    }
}
