import QtQuick
import io.github.wallpaper_rs.app

Item {
    id: stage

    required property var page
    required property real ratio
    property var info: null
    readonly property string selected: page.editor.selected
    readonly property var models: info?.models ?? []

    function load() {
        info = JSON.parse(page.editor.rig("", JSON.stringify({
                        "stage3d": {
                            "time": page.editor.time
                        }
                    })) || "null");
    }

    function view(args) {
        page.run("scene3d", {
                "view": args
            }, true);
    }

    function place(model, field, value) {
        const rounded = value.map(v => Math.round(v * 1000) / 1000);
        if (model.held[field] === "bound")
            return;
        if (model.held[field] === "keyed") {
            page.run("keyframe", {
                    "pointer": model.pointer + "/" + field,
                    "time": page.editor.time,
                    "value": rounded
                }, true);
            return;
        }
        const set = {
            "layer": model.id
        };
        set[field] = rounded;
        page.run("scene3d", {
                "model": {
                    "set": set
                }
            }, true);
    }

    visible: models.length > 0
    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            stage.load();
        }

        function onTimeChanged() {
            stage.load();
        }

        target: stage.page.editor
    }

    SceneStageNavigator {
        anchors.fill: parent
        stage: stage
    }

    SceneLightLines {
        anchors.fill: parent
        ratio: stage.ratio
        dashed: true
        segments: stage.info?.frustum ?? []
    }

    SceneStageCamera {
        anchors.fill: parent
        stage: stage
    }

    SceneStageShadows {
        anchors.fill: parent
        stage: stage
    }

    SceneStagePaths {
        anchors.fill: parent
        stage: stage
    }

    SceneRepeater {
        id: listed2

        readonly property var host: stage

        rows: stage.models

        delegate: SceneStageBones {
            required property int index
            readonly property var modelData: listed2.rows[index]

            anchors.fill: parent
            stage: listed2.host
            model: modelData
        }
    }

    SceneRepeater {
        id: listed

        readonly property var host: stage

        rows: stage.models

        delegate: SceneModelGizmo {
            required property int index
            readonly property var modelData: listed.rows[index]

            anchors.fill: parent
            stage: listed.host
            model: modelData
        }
    }
}
