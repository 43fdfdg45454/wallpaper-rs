import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

Item {
    id: pose

    required property var bones
    readonly property var page: bones.stage.page
    readonly property var mesh: page.mesh
    readonly property var picked: bones.list.find(b => b.name === mesh.bone) ?? null
    readonly property string clip: mesh.layer === bones.model.id && mesh.clip !== "" ? mesh.clip : (bones.model.clips[0] ?? "")
    readonly property real reach: Kirigami.Units.gridUnit * 3
    property var start: null
    property string kind

    function ask(extra) {
        return JSON.parse(page.editor.rig("", JSON.stringify({
                        "bone3d": Object.assign({
                                "layer": bones.model.id,
                                "bone": mesh.bone,
                                "clip": clip,
                                "time": page.editor.time,
                                "clip_time": mesh.time
                            }, extra)
                    })) || "null");
    }

    function grab(kind) {
        if (clip === "") {
            page.run("clip", {
                    "layer": bones.model.id,
                    "id": "clip_1",
                    "add": true
                });
            mesh.layer = bones.model.id;
            mesh.clip = "clip_1";
        }
        pose.kind = kind;
        start = ask({});
        if (start)
            start.head = picked.at.slice();
    }

    function drag(extra) {
        if (!start)
            return;
        const found = ask(Object.assign({
                    "from": start
                }, extra));
        if (!found)
            return;
        page.run("key", {
                "layer": bones.model.id,
                "clip": clip || "clip_1",
                "time": Math.round(mesh.time * 1000) / 1000,
                "bone": mesh.bone,
                "angles": kind === "move" ? start.angles : found.angles,
                "shift": kind === "move" ? found.shift : start.shift
            }, true);
    }

    visible: picked !== null && !page.editor.playing

    Item {
        id: ring

        readonly property real cx: (pose.picked?.at[0] ?? 0) * pose.bones.stage.ratio
        readonly property real cy: (pose.picked?.at[1] ?? 0) * pose.bones.stage.ratio
        property point from

        x: cx - pose.reach
        y: cy - pose.reach
        width: pose.reach * 2
        height: width

        Canvas {
            anchors.fill: parent
            onPaint: {
                const ctx = getContext("2d"), r = pose.reach;
                ctx.reset();
                ctx.strokeStyle = Kirigami.Theme.positiveTextColor;
                ctx.lineWidth = 3;
                ctx.beginPath();
                ctx.arc(r, r, r - 3, 0, 2 * Math.PI);
                ctx.stroke();
                ctx.lineWidth = 1;
                ctx.setLineDash([3, 3]);
                ctx.beginPath();
                ctx.arc(r, r, r * 0.6, 0, 2 * Math.PI);
                ctx.stroke();
            }
        }

        QQC2.ToolTip.visible: hover.hovered
        QQC2.ToolTip.text: App.tr("Drag the ring to turn the bone in the view, inside it to tip it; each drag keys the clip at the playhead")

        HoverHandler {
            id: hover
        }

        DragHandler {
            target: null
            onActiveChanged: {
                if (!active) {
                    pose.start = null;
                    return;
                }
                const p = centroid.pressPosition, r = pose.reach;
                ring.from = p;
                pose.grab(Math.hypot(p.x - r, p.y - r) > r * 0.6 ? "turn" : "tilt");
            }
            onCentroidChanged: {
                if (!active || !pose.start)
                    return;
                const p = centroid.position, f = ring.from, r = pose.reach;
                if (pose.kind === "turn") {
                    const turned = Math.atan2(p.y - r, p.x - r) - Math.atan2(f.y - r, f.x - r);
                    pose.drag({
                            "turn": Math.round(turned * 1800 / Math.PI) / 10
                        });
                } else {
                    pose.drag({
                            "tilt": [Math.round((p.x - f.x) * 5) / 10, Math.round((p.y - f.y) * 5) / 10]
                        });
                }
            }
        }
    }

    SceneKnob {
        at: pose.picked?.at
        ratio: pose.bones.stage.ratio
        square: true
        label: App.tr("Move")
        tip: App.tr("Drag to move the bone in the view; it keys the clip at the playhead")
        onMoved: (to, done) => {
            if (!pose.start)
                pose.grab("move");
            if (pose.start)
                pose.drag({
                        "slide": [to[0] - pose.start.head[0], to[1] - pose.start.head[1]]
                    });
            if (done)
                pose.start = null;
        }
    }
}
