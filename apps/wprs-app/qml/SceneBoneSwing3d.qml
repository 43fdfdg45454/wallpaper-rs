import QtQuick
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: swing

    required property var bones
    readonly property var page: bones.stage.page
    readonly property var list: (bones.model.swing ?? []).filter(s => s.at && s.tip)

    function bound(entry, end, to, done) {
        const found = JSON.parse(page.editor.rig("", JSON.stringify({
                        "bound3d": {
                            "layer": bones.model.id,
                            "bone": entry.bone,
                            "time": page.editor.time,
                            "point": to
                        }
                    })) || "null");
        if (found?.degrees === null || found?.degrees === undefined)
            return;
        const r = entry.rotation.slice();
        r[end] = found.degrees;
        page.run("physics", {
                "layer": bones.model.id,
                "bones": [entry.bone],
                "rotation": [Math.min(r[0], r[1]), Math.max(r[0], r[1])]
            }, !done);
    }

    visible: list.length > 0 && !page.editor.playing

    Repeater {
        model: swing.list

        delegate: Item {
            id: entry

            required property var modelData
            readonly property bool chosen: swing.page.mesh.bone === modelData.bone

            anchors.fill: parent

            SceneLightLines {
                anchors.fill: parent
                ratio: swing.bones.stage.ratio
                tint: entry.chosen ? Kirigami.Theme.positiveTextColor : Kirigami.Theme.neutralTextColor
                dashed: entry.modelData.rotation === null
                segments: entry.modelData.arc.slice(1).map((p, i) => [entry.modelData.arc[i], p]).concat(entry.modelData.rotation ? [[entry.modelData.at, entry.modelData.arc[0]], [entry.modelData.at, entry.modelData.arc[entry.modelData.arc.length - 1]]] : [])
            }

            Repeater {
                model: entry.chosen && entry.modelData.rotation ? [0, 1] : []

                delegate: SceneKnob {
                    required property int modelData

                    at: entry.modelData.ends[modelData]
                    ratio: swing.bones.stage.ratio
                    tint: Kirigami.Theme.neutralTextColor
                    label: modelData === 0 ? App.tr("Least") : App.tr("Most")
                    tip: App.tr("Drag to bound how far the bone swings, in its own plane")
                    onMoved: (to, done) => swing.bound(entry.modelData, modelData, to, done)
                }
            }
        }
    }
}
