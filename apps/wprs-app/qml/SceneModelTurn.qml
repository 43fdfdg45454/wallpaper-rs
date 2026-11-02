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
        tip: App.tr("Drag sideways to turn the model, up and down to tip it")
        onMoved: (to, done) => {
            if (!done)
                return;
            const r = turn.gizmo.three(turn.model.rotation, [0, 0, 0]);
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
        tip: App.tr("Drag away from the model to make it bigger")
        onMoved: (to, done) => {
            if (!done)
                return;
            const at = turn.model.at, corner = [turn.model.box[2], turn.model.box[3]];
            const k = Math.max(0.01, Math.hypot(to[0] - at[0], to[1] - at[1]) / Math.max(Math.hypot(corner[0] - at[0], corner[1] - at[1]), 1e-3));
            turn.gizmo.stage.place(turn.model, "scale", turn.gizmo.three(turn.model.scale, [1, 1, 1]).map(v => v * k));
        }
    }
}
