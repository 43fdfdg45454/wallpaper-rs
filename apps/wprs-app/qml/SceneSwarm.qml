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

    function move(k, at) {
        if (!own)
            return edit("point", {
                    "point": k,
                    "at": at
                });
        page.run("layer", {
                "instance": {
                    "layer": own,
                    "values": {
                        "points": {
                            [String(k)]: at
                        }
                    }
                }
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

    SceneRepeater {
        id: listed2

        readonly property var host: swarm

        rows: swarm.info?.emitters ?? []

        delegate: SceneSwarmShape {
            required property int index
            readonly property var modelData: listed2.rows[index]

            anchors.fill: parent
            swarm: listed2.host
            emitter: modelData
        }
    }

    SceneRepeater {
        id: listed

        rows: swarm.info?.points ?? []

        delegate: SceneKnob {
            required property int index
            readonly property var modelData: listed.rows[index]

            at: modelData.at
            ratio: swarm.ratio
            square: true
            label: modelData.name
            tint: Kirigami.Theme.neutralTextColor
            tip: swarm.own ? App.tr("Drag to move this control point for this layer only") : modelData.bound ? App.tr("This control point follows a value; moving it replaces the value") : App.tr("Drag to move the control point")
            onMoved: (to, done) => swarm.move(Number(modelData.name.slice(1)), modelData.world ? to : swarm.local(to))
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

    SceneSwarmPulls {
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
