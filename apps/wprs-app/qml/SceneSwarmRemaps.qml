import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: remaps

    required property var tools
    readonly property var list: tools.info?.remaps ?? []

    RowLayout {
        Kirigami.Heading {
            Layout.fillWidth: true
            level: 4
            text: App.tr("Starting values from inputs")
        }

        QQC2.ToolButton {
            icon.name: "list-add"
            text: App.tr("Add a remap")
            display: QQC2.AbstractButton.IconOnly
            enabled: remaps.list.length < 8
            onClicked: remaps.tools.edit("remap", {
                "remap": {
                    "input": "random",
                    "in": [0, 1],
                    "output": "size",
                    "out": [1, 8]
                }
            })
        }
    }

    Repeater {
        model: remaps.list

        delegate: SceneSwarmRemap {
            required property var modelData

            Layout.fillWidth: true
            tools: remaps.tools
            row: modelData
        }
    }
}
