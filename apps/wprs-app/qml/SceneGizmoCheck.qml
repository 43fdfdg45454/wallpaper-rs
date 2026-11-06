import QtQuick
import io.github.wallpaper_rs.app

QtObject {
    required property var page

    function read(pointer) {
        return JSON.parse(page.editor.value(pointer) || "null");
    }

    function inputs(item, out) {
        for (const child of item?.children ?? []) {
            if (child.field !== undefined && child.both !== undefined)
                out.push(child);
            inputs(child, out);
        }
        return out;
    }

    function shown(pointer) {
        page.editor.select("/scene/layers/0");
        const found = inputs(page, []).find(input => input.field.pointer === pointer);
        if (!found)
            throw new Error("gizmo: the inspector shows no field " + pointer);
        if (!Array.isArray(found.field.shown))
            throw new Error("gizmo: the inspector field " + pointer + " lost its list: " + typeof found.field.shown);
        return found.both;
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
        if ((gizmo.modelData.corners ?? []).length !== 4)
            throw new Error("gizmo: a tilted layer has no projected outline");
        if (JSON.stringify(shown(base + "position")) !== "[10,20]" || JSON.stringify(shown(base + "tilt")) !== "[12,-4]")
            throw new Error("gizmo: the inspector shows the position and tilt as " + JSON.stringify([shown(base + "position"), shown(base + "tilt")]));
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
        const before = page.view.history.done.length;
        page.editor.hold(true);
        gizmo.commit("tilt", [20, 0]);
        const until = Date.now() + 700;
        while (Date.now() < until) {}
        gizmo.commit("tilt", [30, 5]);
        page.editor.hold(false);
        if (page.view.history.done.length !== before + 1)
            throw new Error("gizmo: a held drag that paused took " + (page.view.history.done.length - before) + " steps");
        gizmo.destroy();
        page.editor.seek(0, false);
        while (page.view.history.done.length > steps)
            page.run("undo");
    }
}
