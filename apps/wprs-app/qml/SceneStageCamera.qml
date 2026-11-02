import QtQuick
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: cam

    required property var stage
    readonly property var handles: stage.info?.handles ?? null
    readonly property var camera: stage.info?.camera ?? null

    function move(field, from, to) {
        const h = handles;
        const d = [to[0] - from[0], to[1] - from[1]];
        const p = (camera?.[field] ?? [0, 0, 0]).map((v, i) => v + (h.right[i] * d[0] - h.upward[i] * d[1]) * h.unit);
        const args = {};
        args[field] = p.map(v => Math.round(v * 1000) / 1000);
        stage.page.run("scene3d", {
            "camera": args
        }, true);
    }

    visible: handles !== null && stage.info?.through === false

    SceneKnob {
        at: cam.handles?.eye ?? null
        ratio: cam.stage.ratio
        label: App.tr("Camera")
        tint: Kirigami.Theme.neutralTextColor
        tip: App.tr("Drag to move the scene camera")
        onMoved: (to, done) => {
            if (done)
                cam.move("eye", cam.handles.eye, to);
        }
    }

    SceneKnob {
        at: cam.handles?.center ?? null
        ratio: cam.stage.ratio
        square: true
        label: App.tr("Aim")
        tint: Kirigami.Theme.neutralTextColor
        tip: App.tr("Drag to move what the scene camera looks at")
        onMoved: (to, done) => {
            if (done)
                cam.move("center", cam.handles.center, to);
        }
    }

    SceneLightLines {
        anchors.fill: parent
        ratio: cam.stage.ratio
        tint: Kirigami.Theme.positiveTextColor
        segments: cam.handles?.eye && cam.handles?.up ? [[cam.handles.eye, cam.handles.up]] : []
    }
}
