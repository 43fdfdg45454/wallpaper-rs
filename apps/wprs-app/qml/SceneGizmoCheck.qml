import QtQuick
import io.github.wallpaper_rs.app

QtObject {
    required property var page

    function read(pointer) {
        return JSON.parse(page.editor.value(pointer) || "null");
    }

    function exercise() {
        const steps = page.view.history.done.length;
        const index = (page.view.gizmos ?? []).findIndex(g => g.pointer === "/scene/layers/0");
        const gizmo = Qt.createComponent("SceneGizmo.qml").createObject(page, {
                "page": page,
                "index": index,
                "ratio": 1
            });
        const base = "/scene/layers/0/transform/";
        gizmo.commit("position", [10, 20]);
        gizmo.commit("tilt", [12, -4]);
        gizmo.commit("z", 30);
        if (JSON.stringify(read(base + "position")) !== "[10,20]" || JSON.stringify(read(base + "tilt")) !== "[12,-4]" || read(base + "z") !== 30)
            throw new Error("gizmo: the handles did not place, tilt and lift the layer: " + page.editor.error);
        for (const t of [0, 1])
            page.run("keyframe", {
                    "pointer": base + "rotation",
                    "time": t,
                    "value": t * 10
                });
        page.editor.seek(0.5, false);
        gizmo.commit("rotation", 45);
        if (!Array.isArray(read(base + "rotation/keys")) || read(base + "rotation/keys").length !== 3)
            throw new Error("gizmo: turning a keyed layer did not add a keyframe at the playhead");
        page.overwrite = true;
        gizmo.commit("rotation", 15);
        page.overwrite = false;
        if (read(base + "rotation") !== 15)
            throw new Error("gizmo: writing over keyframes did not leave a fixed value");
        gizmo.destroy();
        page.editor.seek(0, false);
        while (page.view.history.done.length > steps)
            page.run("undo");
    }
}
