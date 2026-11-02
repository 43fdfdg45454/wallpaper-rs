import QtQuick
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: turn

    required property var gizmo
    readonly property var model: gizmo.model
    readonly property real ratio: gizmo.ratio

    anchors.fill: parent
    visible: gizmo.chosen

    SceneKnob {
        at: [turn.model.at[0], turn.model.box[1] - 24 / turn.ratio]
        ratio: turn.ratio
        label: App.tr("Turn")
        tint: Kirigami.Theme.neutralTextColor
        tip: turn.gizmo.free("rotation") ? App.tr("Drag sideways to turn the model, up and down to tip it") : App.tr("The rotation follows a binding; change it in the inspector")
        opacity: turn.gizmo.free("rotation") ? 1 : 0.4
        onMoved: (to, done) => {
            if (!done || !turn.gizmo.free("rotation"))
                return;
            const r = turn.gizmo.three("rotation");
            r[1] += (to[0] - turn.model.at[0]) * 0.5;
            r[0] += (to[1] - (turn.model.box[1] - 24 / turn.ratio)) * 0.5;
            turn.gizmo.stage.place(turn.model, "rotation", r);
        }
    }

    SceneKnob {
        at: [turn.model.box[2], turn.model.box[3]]
        ratio: turn.ratio
        square: true
        label: App.tr("Size")
        tint: Kirigami.Theme.neutralTextColor
        tip: turn.gizmo.free("scale") ? App.tr("Drag away from the model to make it bigger") : App.tr("The scale follows a binding; change it in the inspector")
        opacity: turn.gizmo.free("scale") ? 1 : 0.4
        onMoved: (to, done) => {
            if (!done || !turn.gizmo.free("scale"))
                return;
            const at = turn.model.at, corner = [turn.model.box[2], turn.model.box[3]];
            const k = Math.max(0.01, Math.hypot(to[0] - at[0], to[1] - at[1]) / Math.max(Math.hypot(corner[0] - at[0], corner[1] - at[1]), 1e-3));
            turn.gizmo.stage.place(turn.model, "scale", turn.gizmo.three("scale").map(v => v * k));
        }
    }
}
