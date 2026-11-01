import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

ColumnLayout {
    id: weights

    required property var host
    readonly property var mesh: host.page.mesh
    readonly property bool painting: mesh.weighing !== null && mesh.rigging === host.layerId
    readonly property var modes: ["add", "subtract", "smooth", "normalise"]
    property var locked: []

    function brush() {
        return {
            "mode": modes[mode.currentIndex],
            "by": Math.round(strength.value * 100) / 100,
            "radius": Math.round(size.value * 1000) / 1000,
            "falloff": Math.round(soft.value * 100) / 100,
            "lock": locked.filter(b => b !== mesh.bone && host.bones.some(x => x.name === b))
        };
    }

    function update() {
        if (painting)
            mesh.weighing = brush();
    }

    visible: host.editable && host.bones.length > 0
    onLockedChanged: update()

    QQC2.Button {
        text: App.tr("Paint weights on the preview")
        icon.name: "draw-brush"
        checkable: true
        checked: weights.painting
        onToggled: {
            if (checked && weights.mesh.bone === "")
                weights.mesh.bone = weights.host.bones[0].name;
            weights.mesh.tool(weights.host.layerId, "weights", checked ? weights.brush() : null);
        }
    }

    GridLayout {
        visible: weights.painting
        columns: 2

        QQC2.Label {
            text: App.tr("Bone")
        }

        QQC2.ComboBox {
            Layout.fillWidth: true
            model: weights.host.bones.map(b => b.name)
            currentIndex: model.indexOf(weights.mesh.bone)
            onActivated: index => weights.mesh.bone = model[index]
        }

        QQC2.Label {
            text: App.tr("Brush")
        }

        QQC2.ComboBox {
            id: mode

            Layout.fillWidth: true
            model: [App.tr("Add"), App.tr("Subtract"), App.tr("Smooth"), App.tr("Normalise")]
            onActivated: weights.update()
        }

        QQC2.Label {
            text: App.tr("Strength")
        }

        QQC2.Slider {
            id: strength

            Layout.fillWidth: true
            value: 0.3
            onMoved: weights.update()
        }

        QQC2.Label {
            text: App.tr("Brush size")
        }

        QQC2.Slider {
            id: size

            Layout.fillWidth: true
            from: 0.01
            to: 0.5
            value: 0.08
            onMoved: weights.update()
        }

        QQC2.Label {
            text: App.tr("Soft edge")
        }

        QQC2.Slider {
            id: soft

            Layout.fillWidth: true
            value: 0.5
            onMoved: weights.update()
        }

        QQC2.Label {
            text: App.tr("Show")
        }

        QQC2.ComboBox {
            Layout.fillWidth: true
            model: [App.tr("Weight of the bone"), App.tr("Bone that leads each part")]
            currentIndex: weights.mesh.view === "owner" ? 1 : 0
            onActivated: index => weights.mesh.view = index === 1 ? "owner" : "heat"
        }
    }

    QQC2.Label {
        visible: weights.painting
        text: App.tr("Locked bones")
    }

    Flow {
        Layout.fillWidth: true
        visible: weights.painting

        Repeater {
            model: weights.host.bones.filter(b => b.name !== weights.mesh.bone)

            delegate: QQC2.CheckBox {
                required property var modelData

                text: modelData.name
                checked: weights.locked.includes(modelData.name)
                onToggled: weights.locked = checked ? weights.locked.concat([modelData.name]) : weights.locked.filter(b => b !== modelData.name)
            }
        }
    }

    RowLayout {
        QQC2.Label {
            Layout.fillWidth: true
            wrapMode: Text.Wrap
            text: weights.host.info.auto ? App.tr("Weights follow the bones until you paint them.") : App.tr("Painted weights stay when bones change.")
        }

        QQC2.Button {
            text: App.tr("Auto weights")
            icon.name: "view-refresh"
            onClicked: weights.host.page.run("auto_weights", {
                "layer": weights.host.layerId
            })
        }
    }
}
