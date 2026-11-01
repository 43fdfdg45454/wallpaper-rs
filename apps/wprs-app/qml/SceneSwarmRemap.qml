import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

GridLayout {
    id: remap

    required property var tools
    required property var row
    readonly property var value: row.remap
    readonly property var inputs: tools.info?.inputs ?? []
    readonly property var outputs: tools.info?.outputs ?? []
    readonly property var domains: tools.info?.domains ?? ({})
    readonly property var reach: domains[value.input.split(":")[0]] ?? [0, 1]
    readonly property var span: domains[value.output] ?? [0, 1]
    readonly property var ops: ["set", "mul", "add", "sub"]
    readonly property var waves: ["none", "sine", "noise"]

    function set(change, gesture) {
        remap.tools.edit("remap", {
            "index": row.index,
            "remap": Object.assign({}, value, change)
        }, gesture);
    }

    columns: 2

    QQC2.ComboBox {
        Layout.fillWidth: true
        model: remap.inputs
        currentIndex: remap.inputs.indexOf(remap.value.input)
        onActivated: index => remap.set({
                "input": remap.inputs[index]
            })
    }

    QQC2.ComboBox {
        Layout.fillWidth: true
        model: remap.outputs
        currentIndex: remap.outputs.indexOf(remap.value.output)
        onActivated: index => remap.set({
                "output": remap.outputs[index]
            })
    }

    QQC2.RangeSlider {
        id: input

        Layout.fillWidth: true
        from: Math.min(remap.reach[0], remap.value.in[0], remap.value.in[1])
        to: Math.max(remap.reach[1], remap.value.in[0], remap.value.in[1])
        first.value: remap.value.in[0]
        second.value: remap.value.in[1]
        first.onMoved: remap.set({
            "in": [first.value, second.value].map(v => Math.round(v * 100) / 100)
        }, true)
        second.onMoved: remap.set({
            "in": [first.value, second.value].map(v => Math.round(v * 100) / 100)
        }, true)
    }

    QQC2.RangeSlider {
        Layout.fillWidth: true
        from: Math.min(remap.span[0], remap.value.out[0], remap.value.out[1])
        to: Math.max(remap.span[1], remap.value.out[0], remap.value.out[1])
        first.value: remap.value.out[0]
        second.value: remap.value.out[1]
        first.onMoved: remap.set({
            "out": [first.value, second.value].map(v => Math.round(v * 100) / 100)
        }, true)
        second.onMoved: remap.set({
            "out": [first.value, second.value].map(v => Math.round(v * 100) / 100)
        }, true)
    }

    QQC2.CheckBox {
        text: App.tr("Clamp the input")
        checked: remap.value.clamp_in === true
        onToggled: remap.set({
            "clamp_in": checked
        })
    }

    QQC2.CheckBox {
        text: App.tr("Clamp the output")
        checked: remap.value.clamp_out === true
        onToggled: remap.set({
            "clamp_out": checked
        })
    }

    QQC2.ComboBox {
        Layout.fillWidth: true
        model: [App.tr("Set"), App.tr("Multiply"), App.tr("Add"), App.tr("Subtract")]
        currentIndex: Math.max(0, remap.ops.indexOf(remap.value.op ?? "set"))
        onActivated: index => remap.set({
                "op": remap.ops[index]
            })
    }

    QQC2.ComboBox {
        Layout.fillWidth: true
        model: [App.tr("Straight"), App.tr("Sine wave"), App.tr("Noise")]
        currentIndex: Math.max(0, remap.waves.indexOf(remap.value.wave ?? "none"))
        onActivated: index => remap.set({
                "wave": remap.waves[index]
            })
    }

    SceneCurvePlot {
        Layout.fillWidth: true
        Layout.columnSpan: 2
        curve: remap.row.curve
        marks: remap.value.in
    }

    QQC2.Button {
        Layout.columnSpan: 2
        icon.name: "edit-delete"
        text: App.tr("Remove this remap")
        onClicked: remap.tools.edit("remap", {
            "index": remap.row.index,
            "remove": true
        })
    }
}
