import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: tool

    required property var page
    readonly property var masks: page.values.masks ?? {}
    readonly property var kinds: ["linear", "radial", "luminance", "depth", "brush"]
    readonly property var start: ({
            "linear": [0.5, 0, 0.5, 0.5],
            "radial": [0.5, 0.5, 0.3, 0.3, 0.5],
            "luminance": [0, 0.5, 0.1],
            "depth": [0, 0.5, 0.1],
            "brush": [0.03, 0.5]
        })
    readonly property var labels: ({
            "linear": [App.tr("Start X"), App.tr("Start Y"), App.tr("End X"), App.tr("End Y")],
            "radial": [App.tr("Center X"), App.tr("Center Y"), App.tr("Width"), App.tr("Height"), App.tr("Feather")],
            "luminance": [App.tr("Darkest"), App.tr("Brightest"), App.tr("Softness")],
            "depth": [App.tr("Farthest"), App.tr("Nearest"), App.tr("Softness")],
            "brush": [App.tr("Brush size"), App.tr("Hardness")]
        })
    property string kind: "linear"
    property var numbers: start.linear
    property bool invert
    property bool erase

    onMasksChanged: {
        if (masks[field.text] && masks[field.text].kind === kind && kind !== "brush")
            numbers = masks[field.text].values;
    }

    Binding {
        target: tool.page
        property: "brush"
        value: tool.kind === "brush" && field.text !== "" ? {
            "name": field.text,
            "radius": tool.numbers[0],
            "hardness": tool.numbers[1],
            "erase": tool.erase,
            "invert": tool.invert
        } : null
    }

    function pick(name) {
        field.text = name;
        page.mask = name;
        kind = masks[name].kind;
        numbers = kind === "brush" ? start.brush : masks[name].values;
        invert = masks[name].invert;
    }

    function send(name, kind, numbers, invert) {
        page.mask = name;
        if (name && kind === "brush" && masks[name]?.kind === "brush" && masks[name].invert !== invert)
            page.run("set", {
                "changes": ["masks." + name + ".invert=" + invert]
            });
        else if (name && kind !== "brush")
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
        model: [App.tr("Linear gradient"), App.tr("Radial gradient"), App.tr("Brightness range"), App.tr("Depth range"), App.tr("Painted with a brush")]
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
            to: tool.kind === "radial" && index > 1 ? 2 : tool.kind === "brush" && index === 0 ? 0.5 : 1
            value: tool.numbers[index] ?? 0
            onMoved: value => {
                tool.numbers = tool.numbers.map((v, i) => i === index ? value : v);
                tool.send(field.text, tool.kind, tool.numbers, tool.invert);
            }
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: tool.kind === "linear" || tool.kind === "radial"
        text: App.tr("Drag the handles on the picture to place the mask.")
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: tool.kind === "brush"
        text: field.text === "" ? App.tr("Name the mask, then paint on the picture.") : App.tr("Paint on the picture; each stroke is one step to undo.")
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
    }

    QQC2.CheckBox {
        visible: tool.kind === "brush"
        text: App.tr("Erase")
        checked: tool.erase
        onToggled: tool.erase = checked
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
