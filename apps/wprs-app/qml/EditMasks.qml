import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: tool

    required property var page
    readonly property var masks: page.values.masks ?? {}
    readonly property var kinds: ["linear", "radial", "luminance"]
    readonly property var start: ({
            "linear": [0.5, 0, 0.5, 0.5],
            "radial": [0.5, 0.5, 0.3, 0.3, 0.5],
            "luminance": [0, 0.5, 0.1]
        })
    readonly property var labels: ({
            "linear": [App.tr("Start X"), App.tr("Start Y"), App.tr("End X"), App.tr("End Y")],
            "radial": [App.tr("Center X"), App.tr("Center Y"), App.tr("Width"), App.tr("Height"), App.tr("Feather")],
            "luminance": [App.tr("Darkest"), App.tr("Brightest"), App.tr("Softness")]
        })
    property string kind: "linear"
    property var numbers: start.linear
    property bool invert

    function pick(name) {
        field.text = name;
        kind = masks[name].kind;
        numbers = masks[name].values;
        invert = masks[name].invert;
    }

    function send(name, kind, numbers, invert) {
        if (name)
            page.run("mask", {
                "name": name,
                "spec": kind + ":" + numbers.join(","),
                "invert": invert
            });
    }

    Flow {
        Layout.fillWidth: true
        spacing: Kirigami.Units.smallSpacing

        Repeater {
            model: Object.keys(tool.masks)

            delegate: Kirigami.Chip {
                required property string modelData

                text: modelData
                checkable: false
                onClicked: tool.pick(modelData)
                onRemoved: tool.page.run("mask", {
                        "name": modelData,
                        "spec": "none"
                    })
            }
        }
    }

    QQC2.TextField {
        id: field

        Layout.fillWidth: true
        placeholderText: App.tr("Name of the mask")
        onEditingFinished: tool.send(text, tool.kind, tool.numbers, tool.invert)
    }

    QQC2.ComboBox {
        Layout.fillWidth: true
        model: [App.tr("Linear gradient"), App.tr("Radial gradient"), App.tr("Brightness range")]
        currentIndex: tool.kinds.indexOf(tool.kind)
        onActivated: index => {
            tool.kind = tool.kinds[index];
            tool.numbers = tool.start[tool.kind];
            tool.send(field.text, tool.kind, tool.numbers, tool.invert);
        }
    }

    Repeater {
        model: tool.labels[tool.kind]

        delegate: EditSlider {
            required property string modelData
            required property int index

            text: modelData
            to: tool.kind === "radial" && index > 1 ? 2 : 1
            value: tool.numbers[index] ?? 0
            onMoved: value => {
                tool.numbers = tool.numbers.map((v, i) => i === index ? value : v);
                tool.send(field.text, tool.kind, tool.numbers, tool.invert);
            }
        }
    }

    QQC2.CheckBox {
        text: App.tr("Invert")
        checked: tool.invert
        onToggled: {
            tool.invert = checked;
            tool.send(field.text, tool.kind, tool.numbers, tool.invert);
        }
    }
}
