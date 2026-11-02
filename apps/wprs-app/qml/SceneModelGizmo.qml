import QtQuick
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: gizmo

    required property var stage
    required property var model
    readonly property real ratio: stage.ratio
    readonly property bool chosen: stage.selected === model.pointer
    readonly property var tints: [Kirigami.Theme.negativeTextColor, Kirigami.Theme.positiveTextColor, Kirigami.Theme.linkColor]

    function three(field) {
        return model.now[field].slice();
    }

    function free(field) {
        return model.held[field] !== "bound";
    }

    function slide(k, to, done) {
        const at = model.at, arm = model.arms[k];
        const d = [arm[0] - at[0], arm[1] - at[1]];
        const along = ((to[0] - at[0]) * d[0] + (to[1] - at[1]) * d[1]) / Math.max(d[0] * d[0] + d[1] * d[1], 1e-6);
        const p = three("position");
        p[k] += (along - 1) * model.length;
        if (done)
            stage.place(model, "position", p);
    }

    Rectangle {
        x: gizmo.model.box[0] * gizmo.ratio
        y: gizmo.model.box[1] * gizmo.ratio
        width: (gizmo.model.box[2] - gizmo.model.box[0]) * gizmo.ratio
        height: (gizmo.model.box[3] - gizmo.model.box[1]) * gizmo.ratio
        color: "transparent"
        border.color: gizmo.chosen ? Kirigami.Theme.highlightColor : Kirigami.Theme.disabledTextColor
        opacity: gizmo.chosen ? 0.9 : 0.4

        TapHandler {
            onTapped: gizmo.stage.page.editor.select(gizmo.model.pointer)
        }
    }

    SceneLightLines {
        anchors.fill: parent
        visible: gizmo.chosen
        ratio: gizmo.ratio
        segments: gizmo.model.arms.map(arm => [gizmo.model.at, arm])
    }

    Repeater {
        model: gizmo.chosen ? gizmo.model.arms : []

        delegate: SceneKnob {
            required property var modelData
            required property int index

            at: modelData
            ratio: gizmo.ratio
            label: ["X", "Y", "Z"][index]
            tint: gizmo.tints[index]
            tip: gizmo.free("position") ? App.tr("Drag to move the model along this direction") : App.tr("The position follows a binding; change it in the inspector")
            opacity: gizmo.free("position") ? 1 : 0.4
            onMoved: (to, done) => {
                if (gizmo.free("position"))
                    gizmo.slide(index, to, done);
            }
        }
    }

    SceneModelTurn {
        gizmo: gizmo
    }
}
