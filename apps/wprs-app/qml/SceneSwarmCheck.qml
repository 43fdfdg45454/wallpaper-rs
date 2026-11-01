import QtQuick
import io.github.wallpaper_rs.app

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

    function edit(what, args) {
        page.run("particles", {
                [what]: Object.assign({
                        "system": "flakes"
                    }, args)
            });
    }

    function tend() {
        edit("collider", {
                "collider": {
                    "bounds": {
                        "circle": {
                            "center": [960, 540],
                            "radius": 200,
                            "inside": true
                        }
                    }
                }
            });
        edit("response", {
                "index": 0,
                "response": "stick"
            });
        edit("force", {
                "force": {
                    "keep_distance": {
                        "target": "p1",
                        "distance": 80,
                        "strength": 3
                    }
                }
            });
        edit("force", {
                "force": {
                    "remap": {
                        "input": "age",
                        "in": [0, 4],
                        "output": "alpha",
                        "out": [1, 0]
                    }
                }
            });
        edit("boids", {
                "boids": {
                    "radius": 50
                }
            });
        edit("strand", {
                "strand": "rope",
                "settings": {
                    "width": 5
                }
            });
        const shown = ask();
        if (shown.colliders[0]?.response !== "stick" || shown.keeps.length !== 1 || shown.boids?.radius !== 50 || shown.strands.rope?.width !== 5 || shown.operators.length !== 1)
            throw new Error("particles: the handles do not show colliders, kept distances, flocks, operators and ropes");
        SceneSwarmCensus.measure(page, "flakes", null);
        if (!(SceneSwarmCensus.census?.alive > 0) || SceneSwarmCensus.census.levels.length !== 4)
            throw new Error("particles: the budget does not count the particles");
        SceneSwarmCensus.quality = "q1";
        SceneSwarmCensus.again(page);
        const look = JSON.parse(page.editor.look || "{}");
        if (look.quality !== "q1" || look.restart !== 1)
            throw new Error("particles: the preview did not take the quality and the restart");
        SceneSwarmCensus.quality = "q3";
        page.editor.look = "";
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
        if (shown?.emitters[0]?.places?.length !== 8 || shown.remaps.length !== 1 || shown.points.length !== 2)
            throw new Error("particles: the handles of the system do not show its shape, remap and points");
        tend();
        page.editor.select("/scene/layers/1");
        for (let k = 0; k < 9; k++)
            page.run("undo");
    }
}
