import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: remaps

    required property var tools
    property bool steady
    readonly property var list: (steady ? tools.info?.operators : tools.info?.remaps) ?? []
    readonly property var fresh: ({
            "input": steady ? "age" : "random",
            "in": [0, 1],
            "output": "size",
            "out": [1, 8]
        })

    RowLayout {
        Kirigami.Heading {
            Layout.fillWidth: true
            level: 4
            text: remaps.steady ? App.tr("Values changed as particles live") : App.tr("Starting values from inputs")
        }

        QQC2.ToolButton {
            icon.name: "list-add"
            text: App.tr("Add a remap")
            display: QQC2.AbstractButton.IconOnly
            enabled: remaps.steady || remaps.list.length < 8
            onClicked: remaps.steady ? remaps.tools.edit("force", {
                    "force": {
                        "remap": remaps.fresh
                    }
                }) : remaps.tools.edit("remap", {
                    "remap": remaps.fresh
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
            steady: remaps.steady
        }
    }
}
