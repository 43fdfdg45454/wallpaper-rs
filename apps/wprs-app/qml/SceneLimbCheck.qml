import QtQuick

QtObject {
    required property var page

    function run(action, args) {
        page.run(action, args);
        if (page.editor.error !== "")
            throw new Error(action + ": " + page.editor.error);
    }

    function exercise() {
        const mesh = page.mesh;
        page.editor.select(mesh.pointer("puppet"));
        run("bone", {
            "layer": "puppet",
            "name": "tip",
            "parent": "limb",
            "at": [0.9, 0.5]
        });
        mesh.use("puppet", "chain");
        run("ik", {
            "layer": "puppet",
            "root": "root",
            "tip": "tip",
            "limits": {
                "limb": [-30, 30]
            }
        });
        const shown = mesh.ask("puppet", {
            "handles": true
        });
        if (shown.ik.length !== 1 || shown.ik[0].limits.length !== 1 || shown.ik[0].follows !== "point")
            throw new Error("the chain has no handles: " + JSON.stringify(shown));
        run("ik", {
            "layer": "puppet",
            "index": 0,
            "target": "bone:root",
            "bend": "counterclockwise"
        });
        mesh.use("puppet", "bodies");
        run("physics", {
            "layer": "puppet",
            "bones": ["limb"],
            "gravity": 2
        });
        mesh.use("puppet", "rope");
        run("physics", {
            "layer": "puppet",
            "bones": ["rope"],
            "rope": [[0.5, 0.6], [0.6, 0.8]],
            "parent": "limb"
        });
        mesh.reset();
        mesh.use("puppet", "wind");
        run("wind", {
            "angle": 45,
            "strength": 80,
            "gusts": 0.5
        });
        if (Math.round(mesh.ask("", {
            "wind": true
        }).strength) !== 80)
            throw new Error("the wind arrow does not show the wind");
        run("add_layer", {
            "kind": "solid",
            "id": "hat"
        });
        mesh.use("puppet", "attach");
        mesh.attaching = "hat";
        run("attach", {
            "layer": "hat",
            "to": "puppet",
            "point": [0.4, 0.5]
        });
        mesh.part = "body";
        mesh.use("puppet", "parts");
        run("parts", {
            "layer": "puppet",
            "order": {
                "body": 2
            }
        });
        page.editor.select(mesh.pointer("puppet") + "/ik/0");
        if (!page.fields.some(f => f.widget === "refs:bone") || !page.fields.some(f => f.widget === "ref:reach"))
            throw new Error("the chain fields do not pick bones: " + JSON.stringify(page.fields.map(f => f.widget)));
        mesh.use("puppet", "");
        mesh.part = "";
    }
}
