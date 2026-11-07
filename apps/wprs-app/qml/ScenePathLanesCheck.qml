import QtQuick
import io.github.wallpaper_rs.app

QtObject {
    required property var page

    function run(path) {
        page.run("scene3d", {
            "path": path
        });
        if (page.editor.error !== "")
            throw new Error("path lanes: " + page.editor.error);
    }

    function exercise() {
        run({
            "add": {
                "id": "tour"
            }
        });
        for (const time of [0, 1, 2])
            run({
                "key": {
                    "path": "tour",
                    "time": time,
                    "eye": [0, 0.5, 3 - time],
                    "center": [0, 0, 0]
                }
            });
        const lanes = Qt.createComponent("ScenePathLanes.qml").createObject(page, {
                "timeline": {
                    "page": page,
                    "length": 4
                }
            });
        lanes.load();
        const times = () => lanes.paths[0].keys.map(k => k.t);
        if (times().join() !== "0,1,2")
            throw new Error("path lanes: the lane lists " + times());
        const steps = page.view.history.done.length;
        lanes.move("tour", 1, 3);
        lanes.load();
        if (times().join() !== "0,2,3" || page.view.history.done.length !== steps + 1)
            throw new Error("path lanes: moving a key left " + times() + " in " + (page.view.history.done.length - steps) + " steps");
        page.run("undo");
        lanes.destroy();
    }
}
