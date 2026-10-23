import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

ColumnLayout {
    id: tool

    required property var page
    readonly property var geometry: page.edit.geometry ?? {}
    readonly property var flip: geometry.flip ?? [false, false]
    readonly property var aspects: [
        {
            "value": "",
            "text": App.tr("Free")
        }
    ].concat(["16:9", "16:10", "21:9", "4:3", "3:2", "1:1", "9:16"].map(ratio => ({
                "value": ratio,
                "text": ratio
            }))).concat(App.outputs.map(name => ({
                "value": App.state.outputs[name].width + ":" + App.state.outputs[name].height,
                "text": App.tr("Screen %1").arg(name)
            })))

    function turn(rotate, straighten, flip) {
        page.run("crop", {
            "rotate": rotate,
            "straighten": straighten,
            "flip": flip ? (flip[0] ? "h" : "") + (flip[1] ? "v" : "") || "none" : undefined
        });
    }

    QQC2.Label {
        text: App.tr("Shape of the crop")
    }

    QQC2.ComboBox {
        Layout.fillWidth: true
        model: tool.aspects
        textRole: "text"
        valueRole: "value"
        onActivated: {
            tool.page.aspect = currentValue;
            if (currentValue)
                tool.page.run("crop", {
                    "aspect": currentValue
                });
        }
    }

    QQC2.Button {
        text: App.tr("Use the whole picture")
        icon.name: "transform-crop"
        onClicked: tool.page.run("crop", {
                "rect": "none"
            })
    }

    QQC2.Label {
        text: App.tr("Placement of this picture")
    }

    QQC2.ComboBox {
        Layout.fillWidth: true
        model: ["none", "fill", "fit", "stretch", "center", "tile"].map(value => ({
                    "value": value,
                    "text": value === "none" ? App.tr("Like the screen") : Labels.value(value)
                }))
        textRole: "text"
        valueRole: "value"
        currentIndex: Math.max(0, indexOfValue(tool.page.edit.fit ?? "none"))
        onActivated: tool.page.run("fit", {
                "mode": currentValue
            })
    }

    RowLayout {
        QQC2.Button {
            text: App.tr("Rotate left")
            icon.name: "object-rotate-left"
            onClicked: tool.turn(((tool.geometry.rotate ?? 0) + 270) % 360)
        }

        QQC2.Button {
            text: App.tr("Rotate right")
            icon.name: "object-rotate-right"
            onClicked: tool.turn(((tool.geometry.rotate ?? 0) + 90) % 360)
        }
    }

    EditSlider {
        text: App.tr("Straighten (degrees)")
        from: -45
        to: 45
        stepSize: 0.1
        decimals: 1
        value: tool.geometry.straighten ?? 0
        onMoved: value => tool.turn(undefined, value)
    }

    QQC2.CheckBox {
        text: App.tr("Flip horizontally")
        checked: tool.flip[0]
        onToggled: tool.turn(undefined, undefined, [checked, tool.flip[1]])
    }

    QQC2.CheckBox {
        text: App.tr("Flip vertically")
        checked: tool.flip[1]
        onToggled: tool.turn(undefined, undefined, [tool.flip[0], checked])
    }
}
