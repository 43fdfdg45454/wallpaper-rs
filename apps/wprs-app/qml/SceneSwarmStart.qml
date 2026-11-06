import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: start

    required property var tools
    readonly property var init: {
        tools.info;
        return JSON.parse(tools.page.editor.value(SceneSwarmKnobs.pointer(tools.system, "/init")) || "null") ?? {};
    }
    readonly property var rows: [["life", App.tr("Life in seconds"), 0, 30, 0.1], ["speed", App.tr("Speed"), 0, 2000, 1], ["angle", App.tr("Direction"), -360, 360, 1], ["size", App.tr("Size"), 0, 500, 1], ["rotation", App.tr("Turn"), -360, 360, 1], ["spin", App.tr("Spin"), -1080, 1080, 1], ["alpha", App.tr("Alpha"), 0, 1, 0.01]]

    Kirigami.Heading {
        level: 4
        text: App.tr("Starting ranges")
    }

    Repeater {
        model: start.rows

        delegate: SceneSwarmSpread {
            required property var modelData

            objectName: "spread_" + modelData[0]
            Layout.fillWidth: true
            tools: start.tools
            row: modelData
            value: start.init[modelData[0]] ?? null
        }
    }

    SceneSwarmTint {
        objectName: "tint"
        Layout.fillWidth: true
        tools: start.tools
        init: start.init
    }
}
