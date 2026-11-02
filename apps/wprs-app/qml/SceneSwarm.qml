import QtQuick
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: swarm

    required property var page
    required property real ratio
    readonly property string pointer: page.editor.selected
    readonly property var node: (page.view.layers ?? []).find(item => item.pointer === pointer) ?? null
    readonly property string own: node?.type === "particles" ? node.id ?? "" : ""
    readonly property string system: {
        const found = /^\/scene\/particles\/([^\/]+)/.exec(pointer);
        if (found)
            return found[1].replace(/~1/g, "/").replace(/~0/g, "~");
        return own ? JSON.parse(page.editor.value(node.pointer + "/system") || "\"\"") : "";
    }
    property var info: null

    function load() {
        const q = {
            "particles": {
                "layer": own || null,
                "time": page.editor.time
            }
        };
        info = system ? JSON.parse(page.editor.rig(system, JSON.stringify(q)) || "null") : null;
    }

    function local(p) {
        const m = info.inverse;
        return [Math.round((m[0] * p[0] + m[2] * p[1] + m[4]) * 10) / 10, Math.round((m[1] * p[0] + m[3] * p[1] + m[5]) * 10) / 10];
    }

    function edit(what, args) {
        page.run("particles", {
                [what]: Object.assign({
                        "system": system
                    }, args)
            }, true);
    }

    function forceOf(i) {
        return JSON.parse(page.editor.value(SceneSwarmKnobs.pointer(system, "/forces/" + i)) || "null");
    }

    function shapeOf(e) {
        return JSON.parse(page.editor.value(SceneSwarmKnobs.pointer(system, "/emitters/" + e + "/shape")) || "null");
    }

    visible: info !== null
    onSystemChanged: load()
    onOwnChanged: load()
    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            swarm.load();
        }

        target: swarm.page.editor
    }

    Repeater {
        model: swarm.info?.emitters ?? []

        delegate: SceneSwarmShape {
            required property var modelData

            anchors.fill: parent
            swarm: swarm
            emitter: modelData
        }
    }

    Repeater {
        model: swarm.info?.points ?? []

        delegate: SceneKnob {
            required property var modelData

            at: modelData.at
            ratio: swarm.ratio
            square: true
            label: modelData.name
            tint: Kirigami.Theme.neutralTextColor
            tip: modelData.bound ? App.tr("This control point follows a value; moving it replaces the value") : App.tr("Drag to move the control point")
            onMoved: (to, done) => swarm.edit("point", {
                        "point": Number(modelData.name.slice(1)),
                        "at": modelData.world ? to : swarm.local(to)
                    })
        }
    }

    SceneSwarmFocal {
        anchors.fill: parent
        swarm: swarm
    }

    SceneSwarmWalls {
        anchors.fill: parent
        swarm: swarm
    }

    SceneSwarmKeeps {
        anchors.fill: parent
        swarm: swarm
    }

    SceneSwarmSwirl {
        anchors.fill: parent
        swarm: swarm
    }

    SceneSwarmLive {
        anchors.fill: parent
        swarm: swarm
    }

    SceneSwarmShift {
        anchors.fill: parent
        swarm: swarm
    }
}
