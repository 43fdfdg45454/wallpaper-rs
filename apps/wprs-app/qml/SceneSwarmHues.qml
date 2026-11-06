import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: hues

    required property var tint

    Rectangle {
        Layout.fillWidth: true
        Layout.preferredHeight: Kirigami.Units.gridUnit / 2

        gradient: Gradient {
            orientation: Gradient.Horizontal

            GradientStop {
                position: 0
                color: Qt.hsva(tint.hues.hue[0] / 360, tint.hues.sat[1], tint.hues.val[1], 1)
            }

            GradientStop {
                position: 0.5
                color: Qt.hsva((tint.hues.hue[0] + tint.hues.hue[1]) / 720, tint.hues.sat[1], tint.hues.val[1], 1)
            }

            GradientStop {
                position: 1
                color: Qt.hsva(Math.min(tint.hues.hue[1], 359.9) / 360, tint.hues.sat[1], tint.hues.val[1], 1)
            }
        }
    }

    Repeater {
        model: [["hue", App.tr("Hue"), 360, 1], ["sat", App.tr("Saturation"), 1, 0.01], ["val", App.tr("Value"), 1, 0.01]]

        delegate: RowLayout {
            required property var modelData

            QQC2.Label {
                Layout.preferredWidth: 110
                text: modelData[1]
            }

            QQC2.RangeSlider {
                Layout.fillWidth: true
                from: 0
                to: modelData[2]
                stepSize: modelData[3]
                first.value: tint.hues[modelData[0]][0] ?? 0
                second.value: tint.hues[modelData[0]][1] ?? modelData[2]
                first.onMoved: tint.hue(modelData[0], [Number(first.value.toFixed(2)), Number(second.value.toFixed(2))], true)
                second.onMoved: tint.hue(modelData[0], [Number(first.value.toFixed(2)), Number(second.value.toFixed(2))], true)
            }
        }
    }

    RowLayout {

        QQC2.CheckBox {
            text: App.tr("Pick the hue among steps")
            checked: (tint.hues.steps ?? 0) > 0
            onToggled: {
                const h = Object.assign({}, tint.hues);
                if (checked)
                    h.steps = 6;
                else
                    delete h.steps;
                tint.put("hsv", h, false);
            }
        }

        QQC2.SpinBox {
            enabled: (tint.hues.steps ?? 0) > 0
            from: 1
            to: 360
            value: tint.hues.steps ?? 6
            onValueModified: tint.hue("steps", value, false)
        }
    }
}
