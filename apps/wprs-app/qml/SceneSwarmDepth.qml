import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

GridLayout {
    id: deep

    required property var tools
    required property var focal
    required property var depth

    columns: 2

    QQC2.CheckBox {
        Layout.columnSpan: 2
        text: App.tr("Perspective")
        checked: deep.focal !== null && deep.focal !== undefined
        onToggled: deep.tools.edit("perspective", checked ? {
            "focal": 800,
            "depth": [0, 400]
        } : {
            "off": true
        })
    }

    QQC2.Label {
        visible: !!deep.focal
        text: App.tr("Focal length")
    }

    QQC2.Slider {
        Layout.fillWidth: true
        visible: !!deep.focal
        from: 50
        to: 4000
        value: deep.focal?.focal ?? 800
        onMoved: deep.tools.edit("perspective", {
            "focal": Math.round(value)
        }, true)
    }

    Repeater {
        model: deep.focal ? [["depth", App.tr("Depth")], ["depth_speed", App.tr("Depth speed")]] : []

        delegate: RowLayout {
            id: span

            required property var modelData
            readonly property var held: deep.depth[modelData[0] === "depth" ? "depth" : "speed"] ?? [0, 0]

            Layout.columnSpan: 2

            QQC2.Label {
                text: span.modelData[1]
            }

            QQC2.RangeSlider {
                Layout.fillWidth: true
                from: span.modelData[0] === "depth" ? -2000 : -500
                to: span.modelData[0] === "depth" ? 4000 : 500
                first.value: span.held[0]
                second.value: span.held[1]
                first.onMoved: deep.tools.edit("perspective", {
                    [span.modelData[0]]: [Math.round(first.value), Math.round(second.value)]
                }, true)
                second.onMoved: deep.tools.edit("perspective", {
                    [span.modelData[0]]: [Math.round(first.value), Math.round(second.value)]
                }, true)
            }
        }
    }
}
