pragma Singleton
import QtQuick

QtObject {
    property var info: null
    property bool shown: false

    function load(page, system, layer) {
        info = system ? JSON.parse(page.editor.rig(system, JSON.stringify({
            "swarm_light": {
                "system": system,
                "layer": layer || null
            }
        })) || "null") : null;
    }

    function run(page, what, args, gesture) {
        page.run("particles", {
                [what]: Object.assign({
                        "system": info?.system ?? ""
                    }, args)
            }, gesture === true);
    }
}
