import QtQuick
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

QtObject {
    required property var page

    function read(pointer) {
        return JSON.parse(page.editor.value(pointer) || "null");
    }

    function exercise() {
        const mesh = page.mesh;
        const keys = mesh.pointer("puppet") + "/clips/0/bones/arm/rotation";
        page.run("key", {
            "layer": "puppet",
            "clip": "wave",
            "time": 0,
            "bone": "arm",
            "rotation": 0
        });
        const line = Qt.createComponent("SceneClipLine.qml").createObject(page, {
                "page": page
            });
        line.load();
        const row = line.rows.find(r => r.name === "arm" && r.channel === "rotation");
        line.chosen = {
            "row": row,
            "t": 0
        };
        const curve = Qt.createComponent("SceneClipCurve.qml").createObject(page, {
                "line": line
            });
        curve.parent = QQC2.Overlay.overlay;
        curve.open();
        curve.refit();
        if (!curve.shaped)
            throw new Error("clip curve: the keys of the row take no handles");
        const steps = page.view.history.done.length;
        curve.press("out");
        curve.grab(40, 30);
        curve.grab(60, 20);
        curve.commit();
        if (page.view.history.done.length !== steps + 1 || !Array.isArray(read(keys + "/0/2/out")))
            throw new Error("clip curve: one drag took " + (page.view.history.done.length - steps) + " steps: " + page.editor.error);
        line.load();
        if (!Array.isArray(line.rows.find(r => r.channel === "rotation").keys[0].ease.out))
            throw new Error("clip curve: the row does not list the handle");
        page.run("key", {
            "layer": "puppet",
            "clip": "wave",
            "time": 0,
            "bone": "arm",
            "rotation": 0,
            "ease": [0.25, 0.1, 0.25, 1]
        });
        if (page.editor.error !== "" || read(keys + "/0/2").length !== 4)
            throw new Error("clip curve: a cubic Bézier easing was not kept: " + page.editor.error);
        curve.close();
        curve.destroy();
        line.destroy();
    }
}
