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
    readonly property real time: clip !== "" ? page.clock : 0

    function ask(id, q) {
        return JSON.parse(page.editor.rig(id, JSON.stringify(q ?? {})) || "null");
    }

    function show() {
        const shaped = Object.keys(shapes).length > 0;
        page.editor.look = layer !== "" && (clip !== "" || shaped) ? JSON.stringify({
            "layer": layer,
            "clip": clip !== "" ? clip : null,
            "shapes": shapes
        }) : "";
        page.editor.seek(page.clock, page.editor.playing);
    }

    function tool(id, kind, settings) {
        rigging = settings === null ? "" : id;
        sculpt = kind === "depth" ? settings : null;
        weighing = kind === "weights" ? settings : null;
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
    onLayerChanged: {
        clip = "";
        shapes = {};
        bone = "";
    }
}
