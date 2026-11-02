pragma Singleton
import QtQuick
import io.github.wallpaper_rs.app

QtObject {
    property var census: null
    property bool shown: false
    property int picked: -1

    function measure(page, system, layer) {
        const q = {
            "budget": {
                "layer": layer || null,
                "time": page.editor.time,
                "quality": SceneLook.quality
            }
        };
        census = system ? JSON.parse(page.editor.rig(system, JSON.stringify(q)) || "null") : null;
    }
}
