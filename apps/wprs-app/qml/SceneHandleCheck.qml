import QtQuick
import io.github.wallpaper_rs.app

QtObject {
    required property var page

    function made(name, props) {
        const item = Qt.createComponent(name).createObject(page, props);
        if (!item)
            throw new Error(name + " did not load");
        return item;
    }

    function value(pointer) {
        return JSON.parse(page.editor.value(pointer) || "null");
    }

    function fluid() {
        page.run("add_layer", {
            "kind": "solid",
            "id": "smoke"
        });
        const layer = page.mesh.pointer("smoke");
        const list = [
            {
                "from": [0.2, 0.9],
                "angle": -90,
                "strength": 4
            },
            {
                "from": [0.1, 1],
                "to": [0.9, 1],
                "angle": -90,
                "strength": 6
            }
        ];
        page.run("set", {
            "pointer": layer + "/effects",
            "value": [
                {
                    "effect": "fluid",
                    "params": {
                        "emitters": list
                    }
                }
            ]
        });
        const canvas = made("SceneEmitters.qml", {
            "page": page,
            "pointer": layer,
            "width": 400,
            "height": 200
        });
        if (canvas.spots.length !== 3)
            throw new Error("the emitters have no handles on the canvas: " + JSON.stringify(canvas.spots));
        const line = canvas.spots.find(s => s.end === "to");
        canvas.follow(line, "to", {
            "x": 300,
            "y": 50,
            "width": 0,
            "height": 0
        }, true);
        if (canvas.drawn(canvas.spots.find(s => s.index === 1 && s.end === "from")).to[0] !== 300)
            throw new Error("the line does not follow the dragged end");
        canvas.place(line, 300, 50);
        const point = canvas.spots.find(s => s.index === 0);
        canvas.follow(point, "tip", {
            "x": point.at[0] * 400 + 60,
            "y": point.at[1] * 180,
            "width": 0,
            "height": 0
        }, true);
        if (Math.abs(canvas.drawn(point).angle) > 45)
            throw new Error("the direction line does not follow the grip while it is dragged");
        canvas.aim(point, point.at[0] * 400 + 60, point.at[1] * 200);
        const now = value(layer + "/effects/0/params/emitters");
        if (now[1].to[0] !== 0.75 || now[0].angle !== 0 || !(now[0].strength > 0))
            throw new Error("dragging the handles did not move, turn or strengthen the emitters: " + JSON.stringify(now));
        page.run("keyframe", {
            "pointer": layer + "/effects/0/params/emitters",
            "time": 0
        });
        page.editor.seek(2, false);
        canvas.place(canvas.spots.find(s => s.index === 0), 200, 100);
        const keyed = value(layer + "/effects/0/params/emitters");
        if (!keyed?.keys || keyed.keys.length !== 2)
            throw new Error("a drag on a keyframed list did not add a keyframe: " + JSON.stringify(keyed));
        page.editor.seek(1, false);
        if (Math.abs(canvas.lists[0][0].from[0] - (now[0].from[0] + 0.5) / 2) > 0.01)
            throw new Error("the canvas does not show the emitters between their keyframes: " + JSON.stringify(canvas.lists));
        page.editor.seek(0, false);
        canvas.destroy();
        const host = Qt.createQmlObject("import QtQuick; QtObject { property var value; property var got: null; function commit(v) { got = v; } }", page);
        host.value = now;
        const rows = made("SceneEmitterList.qml", {
            "host": host
        });
        rows.add(true);
        if (host.got?.length !== 3 || host.got[2].to === undefined)
            throw new Error("the emitter list did not add a line emitter: " + JSON.stringify(host.got));
        rows.destroy();
        host.destroy();
    }

    function knobs(events) {
        const shown = page.mesh.ask("puppet", {
            "handles": true
        });
        const before = shown.ik[0].bend ?? "clockwise";
        events.bend(shown);
        const after = page.mesh.ask("puppet", {
            "handles": true
        }).ik[0].bend;
        if (after === before)
            throw new Error("the bend knob did not flip the chain: " + before);
    }

    function depth(events) {
        events.depth();
        if (value(page.mesh.pointer("puppet") + "/depth")?.sort !== false)
            throw new Error("the depth panel did not keep the file order");
    }

    function mapped() {
        const layer = page.mesh.pointer("smoke"), at = layer + "/effects/0/params/emitters";
        page.run("set", {
            "pointer": "/manifest/properties/night",
            "value": {
                "type": "bool",
                "label": "Night",
                "default": false
            }
        });
        page.run("set", {
            "pointer": at,
            "value": {
                "prop": "night",
                "map": {
                    "false": [{ "from": [0.5, 0.9], "strength": 4 }],
                    "true": [{ "from": [0.2, 0.9], "strength": 4 }, { "from": [0.8, 0.9], "strength": 4 }]
                }
            }
        });
        const canvas = made("SceneEmitters.qml", {
            "page": page,
            "pointer": layer,
            "width": 400,
            "height": 200
        });
        if (canvas.spots.length !== 1)
            throw new Error("the emitters of the current value have no handles: " + JSON.stringify(canvas.spots));
        canvas.place(canvas.spots[0], 100, 100);
        if (value(at + "/map/false")[0].from[0] !== 0.25)
            throw new Error("a drag did not move the emitter of the current value: " + JSON.stringify(value(at)));
        const picked = {};
        picked[layer + "/effects/0"] = "true";
        page.emitterEntry = picked;
        if (canvas.spots.length !== 2)
            throw new Error("the picked value does not show its emitters: " + JSON.stringify(canvas.spots));
        canvas.place(canvas.spots[1], 300, 100);
        if (value(at + "/map/true")[1].from[0] !== 0.75 || value(at + "/map/false")[0].from[0] !== 0.25)
            throw new Error("a drag did not move the emitter of the picked value: " + JSON.stringify(value(at)));
        page.emitterEntry = {};
        canvas.destroy();
    }

    function exercise() {
        const events = made("SceneDragCheck.qml", {
            "page": page
        });
        knobs(events);
        depth(events);
        fluid();
        events.emitters(page.mesh.pointer("smoke"));
        events.finish();
        mapped();
        made("SceneTemplateCheck.qml", {
            "page": page
        }).exercise();
    }
}
