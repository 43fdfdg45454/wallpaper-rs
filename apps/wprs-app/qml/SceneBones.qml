import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Item {
    id: bones

    required property var gizmo
    readonly property var mesh: gizmo.page.mesh
    readonly property string layerId: gizmo.modelData.id
    readonly property bool posing: mesh.posing(layerId)
    readonly property bool placing: mesh.rigging === layerId && mesh.sculpt === null && mesh.weighing === null && mesh.mode === ""
    property var rig: ({
            "bones": []
        })
    readonly property var list: rig.bones ?? []

    function load() {
        rig = mesh.ask(layerId) ?? {
            "bones": []
        };
        links.requestPaint();
    }

    function at(point) {
        return [Math.round(point.x / width * 1000) / 1000, Math.round(point.y / height * 1000) / 1000];
    }

    anchors.fill: parent
    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            bones.load();
        }

        target: bones.gizmo.page.editor
    }

    SceneHeat {
        anchors.fill: parent
        host: bones
        visible: bones.mesh.weighing !== null && bones.mesh.rigging === bones.layerId
    }

    Canvas {
        id: links

        anchors.fill: parent
        visible: !bones.posing
        onPaint: {
            const ctx = getContext("2d");
            ctx.reset();
            ctx.strokeStyle = Kirigami.Theme.highlightColor;
            ctx.lineWidth = 2;
            for (const bone of bones.list) {
                const up = bones.list.find(b => b.name === bone.parent);
                if (!up || !bone.at || !up.at)
                    continue;
                ctx.beginPath();
                ctx.moveTo(up.at[0] * width, up.at[1] * height);
                ctx.lineTo(bone.at[0] * width, bone.at[1] * height);
                ctx.stroke();
            }
        }
    }

    DragHandler {
        property var points: []

        target: null
        enabled: bones.mesh.rigging === bones.layerId && (bones.mesh.sculpt !== null || bones.mesh.weighing !== null)
        onActiveChanged: {
            if (!active && points.length > 0 && bones.mesh.sculpt !== null)
                bones.gizmo.page.run("mesh_depth", Object.assign({
                    "layer": bones.layerId,
                    "points": points
                }, bones.mesh.sculpt));
            else if (!active && points.length > 0 && bones.mesh.weighing !== null && bones.mesh.bone !== "")
                bones.gizmo.page.run("weights", Object.assign({}, bones.mesh.weighing, {
                    "layer": bones.layerId,
                    "bone": bones.mesh.bone,
                    "points": points,
                    "lock": bones.mesh.weighing.lock.filter(b => b !== bones.mesh.bone)
                }));
            points = [];
        }
        onCentroidChanged: {
            if (active && points.length < 1024)
                points = points.concat([bones.at(centroid.position)]);
        }
    }

    SceneLimbs {
        id: limbs

        anchors.fill: parent
        host: bones
        visible: !bones.posing
    }

    Repeater {
        model: bones.posing ? [] : bones.list

        delegate: Rectangle {
            id: knob

            required property var modelData

            x: (modelData.at ?? [0.5, 0.5])[0] * bones.width - width / 2
            y: (modelData.at ?? [0.5, 0.5])[1] * bones.height - height / 2
            width: Kirigami.Units.gridUnit * 0.9
            height: width
            radius: width / 2
            color: bones.mesh.bone === modelData.name ? Kirigami.Theme.positiveTextColor : Kirigami.Theme.highlightColor
            border.color: Kirigami.Theme.backgroundColor
            visible: modelData.at !== undefined

            QQC2.Label {
                anchors.left: parent.right
                anchors.verticalCenter: parent.verticalCenter
                text: knob.modelData.name
            }

            TapHandler {
                onTapped: limbs.tapped(knob.modelData.name)
            }

            DragHandler {
                enabled: bones.placing
                onActiveChanged: {
                    if (active)
                        return;
                    bones.gizmo.page.run("bone", {
                        "layer": bones.layerId,
                        "name": knob.modelData.name,
                        "at": bones.at(knob.mapToItem(bones, knob.width / 2, knob.height / 2))
                    });
                }
            }
        }
    }

    ScenePoseHandles {
        anchors.fill: parent
        host: bones
        visible: bones.posing
    }
}
