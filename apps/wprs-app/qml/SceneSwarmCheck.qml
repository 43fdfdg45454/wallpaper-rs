import QtQuick

QtObject {
    id: check

    required property var page

    function ask() {
        return JSON.parse(page.editor.rig("flakes", JSON.stringify({
            "particles": {
                "time": 0
            }
        })) || "null");
    }

    function exercise() {
        page.editor.select("/scene/particles/flakes");
        page.run("particles", {
            "point": {
                "system": "flakes",
                "point": 1,
                "at": [400, 300]
            }
        });
        page.run("particles", {
            "shape": {
                "system": "flakes",
                "emitter": 0,
                "shape": {
                    "around": {
                        "at": "p1",
                        "count": 8,
                        "radius": 60
                    }
                }
            }
        });
        page.run("particles", {
            "remap": {
                "system": "flakes",
                "remap": {
                    "input": "random",
                    "in": [0, 1],
                    "output": "size",
                    "out": [2, 8]
                }
            }
        });
        const shown = ask();
        if (shown?.emitters?.[0]?.places?.length !== 8 || shown.remaps.length !== 1 || shown.points.length !== 2)
            throw new Error("particles: the handles of the system do not show its shape, remap and points");
        page.editor.select("/scene/layers/1");
        page.run("undo");
        page.run("undo");
        page.run("undo");
    }
}
