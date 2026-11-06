import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: instance

    required property var page
    required property var node
    readonly property bool shown: node !== null && node.type === "particles"
    readonly property var values: shown ? (page.view, JSON.parse(page.editor.value(node.pointer + "/instance") || "null")) ?? ({}) : ({})
    readonly property string system: shown ? (page.view, JSON.parse(page.editor.value(node.pointer + "/system") || "\"\"")) : ""
    readonly property var locked: system ? (page.view, JSON.parse(page.editor.value(SceneSwarmKnobs.pointer(system, "/lock")) || "[]")) : []

    function put(values, clear, gesture) {
        page.run("layer", {
                "instance": {
                    "layer": node.id,
                    "values": values,
                    "clear": clear === true
                }
            }, gesture === true);
    }

    visible: shown

    Kirigami.Heading {
        level: 4
        text: App.tr("This layer from its system")
    }

    Repeater {
        model: [["alpha", App.tr("Opacity"), 1, 1], ["size", App.tr("Size, times"), 4, 1], ["count", App.tr("Share of the particles"), 1, 1], ["speed", App.tr("Speed, times"), 4, 1], ["life", App.tr("Life, times"), 4, 1], ["time_scale", App.tr("Pace of time, times"), 4, 1], ["brightness", App.tr("Brightness"), 16, 1]]

        delegate: RowLayout {
            required property var modelData
            readonly property bool own: typeof instance.values[modelData[0]] === "number"
            readonly property bool fixed: instance.locked.includes(modelData[0])

            QQC2.Label {
                text: parent.fixed ? App.tr("%1 (locked by the system)", modelData[1]) : modelData[1]
                opacity: parent.own ? 1 : 0.6
            }

            SceneSlider {
                Layout.fillWidth: true
                enabled: !parent.fixed
                from: 0
                to: Math.max(modelData[2], instance.values[modelData[0]] ?? 0)
                value: instance.values[modelData[0]] ?? modelData[3]
                onMoved: instance.put({
                        [modelData[0]]: Math.round(value * 100) / 100
                    }, false, true)
            }

            QQC2.ToolButton {
                icon.name: "edit-undo"
                text: App.tr("As the system draws it")
                display: QQC2.AbstractButton.IconOnly
                enabled: parent.own
                onClicked: instance.put({
                        [modelData[0]]: null
                    })
            }
        }
    }

    RowLayout {
        QQC2.Label {
            text: App.tr("Tint")
        }

        SceneColor {
            enabled: !instance.locked.includes("color")
            hex: typeof instance.values.color === "string" ? instance.values.color : "#ffffff"
            onPicked: hex => instance.put({
                        "color": hex
                    })
        }

        QQC2.ToolButton {
            icon.name: "edit-undo"
            text: App.tr("As the system draws it")
            display: QQC2.AbstractButton.IconOnly
            enabled: instance.values.color !== undefined
            onClicked: instance.put({
                    "color": null
                })
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        text: (instance.values.points ?? []).length ? App.tr("The control points on the canvas belong to this layer; drag them to move them here only.") : App.tr("Drag a control point on the canvas to give this layer its own.")
    }

    QQC2.Button {
        icon.name: "edit-clear"
        text: App.tr("Draw everything as the system does")
        enabled: Object.keys(instance.values).length > 0
        onClicked: instance.put({}, true)
    }
}
