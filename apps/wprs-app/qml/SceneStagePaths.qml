import QtQuick
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: paths

    required property var stage
    property var info: null
    readonly property var handles: stage.info?.handles ?? null

    function load() {
        info = JSON.parse(stage.page.editor.rig("", JSON.stringify({
                        "paths3d": {
                            "time": stage.page.editor.time
                        }
                    })) || "null");
    }

    function moveKey(path, key, field, to) {
        const h = handles, from = field === "eye" ? key.at : key.aim, k = key["reach_" + field] ?? h.unit;
        const d = [to[0] - from[0], to[1] - from[1]];
        const args = {
            "path": path,
            "time": key.t
        };
        args[field] = key[field].map((v, i) => Math.round((v + (h.right[i] * d[0] - h.upward[i] * d[1]) * k) * 1000) / 1000);
        stage.page.run("scene3d", {
                "path": {
                    "key": args
                }
            }, true);
    }

    visible: handles !== null && (info?.paths ?? []).length > 0
    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            paths.load();
        }

        function onTimeChanged() {
            paths.load();
        }

        target: paths.stage.page.editor
    }

    Repeater {
        model: paths.info?.paths ?? []

        delegate: Item {
            id: one

            required property var modelData

            anchors.fill: parent

            SceneLightLines {
                anchors.fill: parent
                ratio: paths.stage.ratio
                tint: Kirigami.Theme.positiveTextColor
                segments: one.modelData.spline.slice(1).map((p, i) => [one.modelData.spline[i], p])
            }

            Repeater {
                model: one.modelData.keys.filter(k => k.at)

                delegate: SceneKnob {
                    required property var modelData

                    at: modelData.at
                    ratio: paths.stage.ratio
                    label: App.tr("%1 at %2 s", one.modelData.id, modelData.t)
                    tint: Kirigami.Theme.positiveTextColor
                    tip: App.tr("Drag to move this key of the camera path")
                    onMoved: (to, done) => {
                        if (done)
                            paths.moveKey(one.modelData.id, modelData, "eye", to);
                    }
                }
            }

            SceneLightLines {
                anchors.fill: parent
                ratio: paths.stage.ratio
                dashed: true
                tint: Kirigami.Theme.positiveTextColor
                segments: one.modelData.keys.filter(k => k.at && k.aim).map(k => [k.at, k.aim])
            }

            Repeater {
                model: one.modelData.keys.filter(k => k.aim)

                delegate: SceneKnob {
                    required property var modelData

                    at: modelData.aim
                    ratio: paths.stage.ratio
                    square: true
                    label: App.tr("Aim at %1 s", modelData.t)
                    tint: Kirigami.Theme.positiveTextColor
                    tip: App.tr("Drag to move the point this key of the camera path looks at")
                    onMoved: (to, done) => {
                        if (done)
                            paths.moveKey(one.modelData.id, modelData, "center", to);
                    }
                }
            }
        }
    }

    SceneKnob {
        at: paths.info?.playing?.at ?? null
        ratio: paths.stage.ratio
        square: true
        label: App.tr("Camera now")
        tint: Kirigami.Theme.neutralTextColor
        tip: App.tr("Where the paths put the camera at this time")
    }
}
