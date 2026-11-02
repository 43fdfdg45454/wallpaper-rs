import QtQuick
import org.kde.kirigami as Kirigami

Item {
    id: frusta

    required property var stage
    property var info: null
    readonly property bool shown: (JSON.parse(stage.page.editor.value("/scene/ext/editor/debug3d") || "null") ?? "") === "shadows"

    function load() {
        info = shown ? JSON.parse(stage.page.editor.rig("", JSON.stringify({
            "shadows3d": {
                "time": stage.page.editor.time
            }
        })) || "null") : null;
    }

    visible: shown
    onShownChanged: load()
    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            frusta.load();
        }

        target: frusta.stage.page.editor
    }

    Repeater {
        model: frusta.info?.lights ?? []

        delegate: SceneLightLines {
            required property var modelData

            anchors.fill: parent
            ratio: frusta.stage.ratio
            dashed: true
            tint: Kirigami.Theme.negativeTextColor
            segments: modelData.frustum ?? []
        }
    }
}
