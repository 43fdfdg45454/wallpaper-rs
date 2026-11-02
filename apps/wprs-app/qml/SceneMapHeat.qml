import QtQuick
import io.github.wallpaper_rs.app

Image {
    id: heat

    required property var page
    property var shown: null

    function stroke(brush, points) {
        const args = Object.assign({
            "points": points
        }, brush);
        delete args.action;
        if (brush.action === "paint_map")
            delete args.pointer;
        return args;
    }

    function load() {
        if (!visible)
            return;
        const q = {
            "map": {
                "layer": page.brush.layer,
                "map": page.brush.map
            }
        };
        shown = JSON.parse(page.editor.rig(page.brush.layer, JSON.stringify(q)) || "null");
    }

    opacity: 0.6
    smooth: true
    cache: false
    source: shown?.heat ?? ""
    onVisibleChanged: load()

    Connections {
        function onViewChanged() {
            heat.load();
        }

        target: heat.page.editor
    }
}
