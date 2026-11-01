import QtQuick
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami

Item {
    id: bones

    required property var gizmo
    property var rig: ({
            "bones": []
        })
    readonly property var list: rig.bones ?? []

    function load() {
        rig = JSON.parse(gizmo.page.editor.rig(gizmo.modelData.id) || "null") ?? {
            "bones": []
        };
        links.requestPaint();
    }

    anchors.fill: parent
    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            bones.load();
        }

        target: bones.gizmo.page.editor
    }

    Canvas {
        id: links

        anchors.fill: parent
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

    Repeater {
        model: bones.list

        delegate: Rectangle {
            id: knob

            required property var modelData

            x: (modelData.at ?? [0.5, 0.5])[0] * bones.width - width / 2
            y: (modelData.at ?? [0.5, 0.5])[1] * bones.height - height / 2
            width: Kirigami.Units.gridUnit * 0.9
            height: width
            radius: width / 2
            color: Kirigami.Theme.highlightColor
            border.color: Kirigami.Theme.backgroundColor
            visible: modelData.at !== undefined

            QQC2.Label {
                anchors.left: parent.right
                anchors.verticalCenter: parent.verticalCenter
                text: knob.modelData.name
            }

            DragHandler {
                onActiveChanged: {
                    if (active)
                        return;
                    const p = knob.mapToItem(bones, knob.width / 2, knob.height / 2);
                    bones.gizmo.page.run("bone", {
                        "layer": bones.gizmo.modelData.id,
                        "name": knob.modelData.name,
                        "at": [Math.round(p.x / bones.width * 1000) / 1000, Math.round(p.y / bones.height * 1000) / 1000]
                    });
                }
            }
        }
    }
}
