import QtQuick
import QtQuick.Layouts
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: motion

    required property var host
    property int revision: 0
    readonly property var names: host.bones.map(b => b.name)
    readonly property var chains: (revision, listed("/ik", []))
    readonly property var bodies: (revision, listed("/physics", []))
    readonly property var touched: (revision, listed("/hit", []))
    readonly property var depth: (revision, listed("/depth", null))
    readonly property var points: (host.info.points ?? []).map(p => p.name)
    readonly property var targets: ["cursor"].concat(names.concat(points).map(n => "bone:" + n))

    function listed(key, empty) {
        return JSON.parse(host.input.page.editor.value(host.base + key) || "null") ?? empty;
    }

    function put(key, value) {
        if (value === null)
            host.input.page.run("remove", {
                "pointer": host.base + key
            });
        else
            host.input.page.run("set", {
                "pointer": host.base + key,
                "value": value
            });
    }

    function run(action, args) {
        host.input.page.run(action, Object.assign({
            "layer": host.layerId
        }, args));
    }

    function pair(key, value) {
        const out = {};
        out[key] = value;
        return out;
    }

    function without(list, index) {
        return list.filter((_, i) => i !== index);
    }

    Connections {
        function onViewChanged() {
            motion.revision++;
        }

        target: motion.host.input.page.editor
    }

    Kirigami.Separator {
        Layout.fillWidth: true
    }

    SceneRigIk {
        Layout.fillWidth: true
        owner: motion
    }

    SceneRigPhysics {
        Layout.fillWidth: true
        owner: motion
    }

    SceneRigTouch {
        Layout.fillWidth: true
        owner: motion
    }
}
