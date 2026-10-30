import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

ColumnLayout {
    id: tool

    required property var page
    readonly property var levels: page.values.levels ?? {}
    readonly property var owned: ["ev", "wb", "ct", "sa"].filter(id => tool.page.ops.some(op => op.id === id))

    Repeater {
        model: [
            {
                "key": "ev",
                "text": App.tr("Exposure (EV)"),
                "from": -5,
                "to": 5,
                "step": 0.05
            },
            {
                "key": "temperature",
                "text": App.tr("Temperature (K)"),
                "from": 2000,
                "to": 12000,
                "step": 50
            },
            {
                "key": "tint",
                "text": App.tr("Tint"),
                "from": -100,
                "to": 100,
                "step": 1
            },
            {
                "key": "contrast",
                "text": App.tr("Contrast"),
                "from": -100,
                "to": 100,
                "step": 1
            },
            {
                "key": "saturation",
                "text": App.tr("Saturation"),
                "from": -100,
                "to": 100,
                "step": 1
            },
            {
                "key": "vibrance",
                "text": App.tr("Vibrance"),
                "from": -100,
                "to": 100,
                "step": 1
            }
        ]

        delegate: EditSlider {
            required property var modelData

            text: modelData.text
            from: modelData.from
            to: modelData.to
            stepSize: modelData.step
            value: tool.levels[modelData.key] ?? 0
            onMoved: value => tool.page.run("set", {
                    "changes": [modelData.key + "=" + value]
                })
        }
    }

    QQC2.Button {
        text: App.tr("Reset light and color")
        icon.name: "edit-reset"
        enabled: tool.owned.length > 0
        onClicked: tool.page.run("reset", {
            "ids": tool.owned
        })
    }
}
