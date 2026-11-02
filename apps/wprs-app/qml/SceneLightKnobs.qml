import QtQuick
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: knobs

    required property var gizmo
    readonly property var light: gizmo.light
    readonly property real ratio: gizmo.ratio

    visible: gizmo.chosen

    SceneKnob {
        at: knobs.light.z_knob
        ratio: knobs.ratio
        square: true
        label: App.tr("Height %1", Math.round(knobs.light.z))
        tip: App.tr("Drag up or down to raise or lower the light")
        onMoved: (to, done) => knobs.gizmo.put("z", Math.round((knobs.gizmo.at[1] - to[1]) / 0.5))
    }

    SceneKnob {
        at: knobs.light.radius_knob
        ratio: knobs.ratio
        tint: Kirigami.Theme.positiveTextColor
        tip: App.tr("Drag to change where the light ends")
        onMoved: (to, done) => knobs.gizmo.put("radius", Math.max(1, Math.round(Math.hypot(to[0] - knobs.gizmo.at[0], to[1] - knobs.gizmo.at[1]))))
    }

    SceneKnob {
        at: knobs.light.aim ?? null
        ratio: knobs.ratio
        tip: App.tr("Drag to turn the light")
        onMoved: (to, done) => knobs.gizmo.aim(to)
    }

    SceneKnob {
        at: knobs.light.tilt_knob ?? null
        ratio: knobs.ratio
        square: true
        tint: Kirigami.Theme.neutralTextColor
        label: App.tr("Tilt %1°", Math.round(knobs.light.tilt))
        tip: App.tr("Drag along the aim line to tilt the light: nearer the light it faces the canvas, nearer the end it lies flat")
        onMoved: (to, done) => knobs.gizmo.tilt(to)
    }

    Repeater {
        model: knobs.light.type === "spot" ? [[knobs.light.outer[1], false], [knobs.light.inner[1], true]] : []

        delegate: SceneKnob {
            required property var modelData

            at: modelData[0]
            ratio: knobs.ratio
            square: true
            tint: modelData[1] ? Kirigami.Theme.positiveTextColor : Kirigami.Theme.negativeTextColor
            tip: modelData[1] ? App.tr("Drag to widen or narrow the bright middle of the cone") : App.tr("Drag to widen or narrow the cone")
            onMoved: (to, done) => knobs.gizmo.cone(to, modelData[1])
        }
    }

    Repeater {
        model: knobs.light.ends ?? []

        delegate: SceneKnob {
            required property var modelData
            required property int index

            at: modelData
            ratio: knobs.ratio
            square: true
            tip: App.tr("Drag to move this end of the tube")
            onMoved: (to, done) => knobs.gizmo.end(to, knobs.light.ends[1 - index], index === 1)
        }
    }
}
