import QtQuick
import io.github.wallpaper_rs.app

QtObject {
    required property var page

    function read(pointer) {
        return JSON.parse(page.editor.value(pointer) || "null");
    }

    function exercise(check) {
        const base = "/scene/particles/flakes";
        const tools = Qt.createQmlObject("import QtQuick; QtObject { property var info; property var page; property var check; property string system: 'flakes'; function edit(what, args, gesture) { check.edit(what, args); info = check.ask(); } }", page);
        tools.page = page;
        tools.check = check;
        tools.info = check.ask();
        const emission = Qt.createComponent("SceneSwarmEmission.qml").createObject(page, {
                "tools": tools,
                "index": 0
            });
        emission.rate(42, true);
        emission.put({
                "burst": {
                    "count": 5,
                    "every": 2
                },
                "delay": 1,
                "duration": 6,
                "periodic": {
                    "on": [1, 2],
                    "off": [0.5, 1]
                },
                "follow": "cursor",
                "inherit": 0.4
            });
        const e = read(base + "/emitters/0");
        if (e.rate !== 42 || e.burst.every !== 2 || e.delay !== 1 || e.duration !== 6 || e.follow !== "cursor" || e.inherit !== 0.4)
            throw new Error("emission: the emitter did not take its emission: " + JSON.stringify(e) + page.editor.error);
        const lane = Qt.createComponent("SceneSwarmEmitLane.qml").createObject(page, {
                "emission": emission
            });
        if (lane.span < 7 || lane.delay !== 1 || lane.every !== 2 || lane.ranged([1, 2], 3)[0] !== 2.5)
            throw new Error("emission: the lane does not lay out the emission");
        const brief = lane.ranged([1, 3], 1.01);
        if (brief[0] < 1 / 60 || brief[1] < brief[0] || lane.ranged([0.5, 0.5], 0)[0] < 1 / 60)
            throw new Error("emission: a dragged span went below the shortest the scene takes: " + brief);
        emission.put({
                "burst": {
                    "count": 5,
                    "every": 0.02
                },
                "delay": 0,
                "duration": 86400
            });
        lane.width = 400;
        const begun = Date.now(), ticks = lane.ticks(86400);
        if (ticks.length > 1000 || Date.now() - begun > 50)
            throw new Error("emission: a day of bursts drew " + ticks.length + " lines in " + (Date.now() - begun) + " ms");
        lane.destroy();
        emission.destroy();
        const stack = Qt.createComponent("SceneSwarmStack.qml").createObject(page, {
                "tools": tools
            });
        tools.edit("render", {
                "order": "oldest",
                "cycles": 3,
                "frames": "life"
            });
        tools.edit("settings", {
                "lock": ["size", "points"]
            });
        if (stack.render.order !== "oldest" || stack.locked.length !== 2)
            throw new Error("emission: the drawing order or the lock did not show: " + page.editor.error);
        stack.destroy();
        tools.destroy();
        const node = (page.view.layers ?? []).find(l => l.type === "particles");
        const own = Qt.createComponent("SceneSwarmInstance.qml").createObject(page, {
            "page": page,
            "node": node
        });
        own.put({
            "alpha": 0.5,
            "color": "#ff8800"
        });
        if (own.values.alpha !== 0.5 || own.locked.length !== 2 || !own.visible)
            throw new Error("instance: the layer did not take its own values: " + JSON.stringify(own.values) + page.editor.error);
        own.put({}, true);
        if (Object.keys(own.values).length !== 0)
            throw new Error("instance: clearing kept values of the layer");
        own.destroy();
    }
}
