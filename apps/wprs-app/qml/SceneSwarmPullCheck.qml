import QtQuick
import io.github.wallpaper_rs.app

QtObject {
    required property var page

    function read(pointer) {
        return JSON.parse(page.editor.value(pointer) || "null");
    }

    function exercise(check) {
        check.edit("force", {
                "force": {
                    "attractor": {
                        "target": [400, 300],
                        "strength": 50,
                        "radius": 100
                    }
                }
            });
        const swarm = Qt.createQmlObject("import QtQuick; QtObject { property var info; property real ratio: 1; property var page; property var check; function forceOf(i) { return JSON.parse(page.editor.value('/scene/particles/flakes/forces/' + i) || 'null'); } function edit(what, args) { check.edit(what, args); info = check.ask(); } function local(p) { return p; } }", page);
        swarm.page = page;
        swarm.check = check;
        swarm.info = check.ask();
        const pulls = Qt.createComponent("SceneSwarmPulls.qml").createObject(page, {
                "swarm": swarm
            });
        if (pulls.list.length !== 1 || pulls.list[0].rings.length !== 1)
            throw new Error("pulls: the attractor shows no center or ring: " + JSON.stringify(swarm.info.pulls));
        const e = pulls.list[0], index = e.index;
        pulls.reach(e, e.rings[0], [e.at[0] + 2 * e.rings[0].px, e.at[1]]);
        const radius = read("/scene/particles/flakes/forces/" + index + "/attractor/radius");
        if (Math.abs(radius - 200) > 1)
            throw new Error("pulls: dragging the ring did not set the radius: " + radius);
        SceneSwarmGrips.spot(pulls, pulls.list[0], pulls.list[0].ref, [420, 310], (v, p) => v[pulls.list[0].field] = p);
        if (JSON.stringify(read("/scene/particles/flakes/forces/" + index + "/attractor/target")) !== "[420,310]")
            throw new Error("pulls: dragging the center did not move it");
        pulls.destroy();
        swarm.destroy();
    }
}
