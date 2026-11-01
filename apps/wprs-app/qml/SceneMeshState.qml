import QtQuick

QtObject {
    id: mesh

    required property var page
    property string layer
    property string rigging
    property var sculpt: null
    property var weighing: null
    property string view: "heat"
    property string bone
    property string clip
    property var shapes: ({})
    property var pose: null
    property string mode
    property string root
    property string part
    property string attaching
    property string spot
    property var inherit: ["position", "rotation", "scale"]
    property bool still
    readonly property real time: clip !== "" ? page.clock : 0

    function ask(id, q) {
        return JSON.parse(page.editor.rig(id, JSON.stringify(q ?? {})) || "null");
    }

    function show() {
        const shaped = Object.keys(shapes).length > 0;
        const kept = JSON.parse(page.editor.look || "{}");
        const look = layer !== "" && (clip !== "" || shaped || still) ? {
            "layer": layer,
            "clip": clip !== "" ? clip : null,
            "shapes": shapes,
            "still": still
        } : {};
        ["quality", "restart"].filter(k => kept[k] !== undefined).forEach(k => look[k] = kept[k]);
        page.editor.look = Object.keys(look).length > 0 ? JSON.stringify(look) : "";
        page.editor.seek(page.clock, page.editor.playing);
    }

    function tool(id, kind, settings) {
        rigging = settings === null ? "" : id;
        sculpt = kind === "depth" ? settings : null;
        weighing = kind === "weights" ? settings : null;
        mode = "";
    }

    function use(id, kind) {
        tool(id, "", kind === "" ? null : {});
        mode = kind;
        root = "";
    }

    function reset() {
        still = true;
        Qt.callLater(() => still = false);
    }

    function pending(bone) {
        const p = pose && pose.bone === bone ? pose : null;
        const out = {};
        (p?.changed ?? []).forEach(c => out[c] = p[c]);
        return out;
    }

    function pointer(id) {
        return (page.view.layers ?? []).find(l => l.id === id)?.pointer ?? "";
    }

    function posing(id) {
        return clip !== "" && layer === id;
    }

    onClipChanged: {
        pose = null;
        show();
    }
    onShapesChanged: show()
    onStillChanged: show()
    onLayerChanged: {
        clip = "";
        shapes = {};
        bone = "";
        part = "";
        still = false;
    }
}
