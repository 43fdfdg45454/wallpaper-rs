import QtQuick
import QtQuick.Dialogs
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: tool

    required property var page
    readonly property var tools: page.values.tools ?? {}
    readonly property var vignette: tools.vi ?? {}
    readonly property var overlay: tools.ov ?? {}
    readonly property var blends: ["normal", "multiply", "screen", "overlay", "soft_light", "color"]

    Kirigami.Heading {
        level: 4
        text: App.tr("Blur")
    }

    EditSlider {
        text: App.tr("Radius")
        to: 200
        stepSize: 1
        value: tool.tools.bl?.radius ?? 0
        onMoved: value => tool.page.run("blur", {
                "radius": value
            })
    }

    Kirigami.Heading {
        level: 4
        text: App.tr("Vignette")
    }

    Repeater {
        model: [["amount", App.tr("Amount"), -1], ["midpoint", App.tr("Midpoint"), 0], ["roundness", App.tr("Roundness"), 0], ["feather", App.tr("Feather"), 0]]

        delegate: EditSlider {
            required property var modelData

            text: modelData[1]
            from: modelData[2]
            value: tool.vignette[modelData[0]] ?? 0
            onMoved: value => {
                const args = {};
                args[modelData[0]] = value;
                tool.page.run("vignette", args);
            }
        }
    }

    Kirigami.Heading {
        level: 4
        text: App.tr("Color overlay")
    }

    QQC2.Button {
        text: tool.overlay.color ?? App.tr("Choose a color…")
        icon.name: "color-picker"
        onClicked: colors.open()
    }

    QQC2.ComboBox {
        Layout.fillWidth: true
        model: [App.tr("Normal"), App.tr("Multiply"), App.tr("Screen"), App.tr("Overlay"), App.tr("Soft light"), App.tr("Color")]
        currentIndex: Math.max(0, tool.blends.indexOf(tool.overlay.blend))
        onActivated: index => tool.page.run("overlay", {
                "blend": tool.blends[index]
            })
    }

    EditSlider {
        text: App.tr("Opacity")
        value: tool.overlay.opacity ?? 1
        onMoved: value => tool.page.run("overlay", {
                "opacity": value
            })
    }

    QQC2.Label {
        Layout.fillWidth: true
        text: App.tr("Limit an effect to part of the picture with a mask in the list of operations.")
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
    }

    ColorDialog {
        id: colors

        selectedColor: tool.overlay.color ?? "black"
        onAccepted: tool.page.run("overlay", {
            "color": selectedColor.toString()
        })
    }
}
