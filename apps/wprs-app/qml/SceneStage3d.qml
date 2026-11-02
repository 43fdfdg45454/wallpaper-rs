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
        const set = {
            "layer": model.id
        };
        set[field] = value.map(v => Math.round(v * 1000) / 1000);
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

    Repeater {
        model: stage.models

        delegate: SceneModelGizmo {
            required property var modelData

            anchors.fill: parent
            stage: stage
            model: modelData
        }
    }
}
