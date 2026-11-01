pragma Singleton
import QtQuick

QtObject {
    property var census: null
    property string quality: "q3"
    property int restart: 0
    property bool shown: false
    property int picked: -1

    function measure(page, system, layer) {
        const q = {
            "budget": {
                "layer": layer || null,
                "time": page.editor.time,
                "quality": quality
            }
        };
        census = system ? JSON.parse(page.editor.rig(system, JSON.stringify(q)) || "null") : null;
    }

    function preview(page) {
        const look = JSON.parse(page.editor.look || "{}");
        look.quality = quality;
        look.restart = restart;
        page.editor.look = JSON.stringify(look);
        page.editor.seek(page.clock, page.editor.playing);
    }

    function again(page) {
        restart += 1;
        preview(page);
    }
}
