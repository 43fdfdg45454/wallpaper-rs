import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: tune

    required property var panel
    readonly property var mask: panel.mask !== null && typeof panel.mask === "object" ? panel.mask : ({})
    readonly property var channels: ["luminance", "r", "g", "b", "a"]

    visible: panel.source !== ""

    RowLayout {
        visible: tune.panel.source === "texture" || tune.panel.source === "layer"

        QQC2.Label {
            text: App.tr("Channel")
        }

        QQC2.ComboBox {
            model: [App.tr("Brightness"), App.tr("Red"), App.tr("Green"), App.tr("Blue"), App.tr("Alpha")]
            currentIndex: Math.max(0, tune.channels.indexOf(tune.mask.channel ?? (tune.panel.source === "texture" ? "luminance" : "a")))
            onActivated: index => tune.panel.put({
                        "channel": tune.channels[index]
                    })
        }
    }

    QQC2.CheckBox {
        text: App.tr("Invert: apply the effect where the mask is empty")
        checked: tune.mask.invert === true
        onToggled: tune.panel.put({
                "invert": checked
            })
    }

    RowLayout {
        visible: tune.panel.source === "shape"

        QQC2.Label {
            text: App.tr("Soft edge")
        }

        SceneSlider {
            Layout.fillWidth: true
            from: 0
            to: Math.max(200, feather.value)
            stepSize: 1
            value: tune.mask.feather ?? 0
            onMoved: tune.panel.put({
                    "feather": Math.round(value)
                }, true)
        }

        QQC2.SpinBox {
            id: feather

            from: 0
            to: 10000
            editable: true
            value: tune.mask.feather ?? 0
            textFromValue: (v, locale) => App.tr("%1 px", v)
            valueFromText: (text, locale) => parseInt(text) || 0
            onValueModified: tune.panel.put({
                    "feather": value
                })
        }
    }

    RowLayout {
        QQC2.Label {
            text: App.tr("Strength")
        }

        SceneSlider {
            Layout.fillWidth: true
            from: 0
            to: 1
            stepSize: 0.01
            value: typeof tune.mask.strength === "number" ? tune.mask.strength : 1
            enabled: tune.mask.strength === undefined || typeof tune.mask.strength === "number"
            onMoved: tune.panel.put({
                    "strength": Math.round(value * 100) / 100
                }, true)
        }
    }
}
