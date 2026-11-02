import QtQuick
import io.github.wallpaper_rs.app

Item {
    id: lights

    required property var page
    required property real ratio
    property var info: null
    readonly property string selected: page.editor.selected

    function load() {
        const q = {
            "lights": {
                "time": page.editor.time
            }
        };
        info = JSON.parse(page.editor.rig("", JSON.stringify(q)) || "null");
    }

    visible: (info?.lights ?? []).length > 0
    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            lights.load();
        }

        function onTimeChanged() {
            lights.load();
        }

        target: lights.page.editor
    }

    Repeater {
        model: lights.info?.lights ?? []

        delegate: SceneLightGizmo {
            required property var modelData

            anchors.fill: parent
            owner: lights
            light: modelData
        }
    }
}
