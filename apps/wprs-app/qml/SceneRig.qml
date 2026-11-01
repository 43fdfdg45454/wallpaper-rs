import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: rig

    required property var input
    readonly property var page: input.page
    readonly property string base: input.field.pointer.replace(/\/[^/]*$/, "")
    readonly property string layerId: JSON.parse(page.editor.value(base + "/id") || "\"\"") ?? ""
    property var info: ({})
    readonly property var bones: info.bones ?? []
    readonly property var clips: info.clips ?? []
    readonly property bool editable: info.editable === true

    function load() {
        info = page.mesh.ask(layerId) ?? {};
    }

    function bone(args) {
        page.run("bone", Object.assign({
            "layer": rig.layerId
        }, args));
    }

    Component.onCompleted: {
        load();
        page.mesh.layer = layerId;
    }

    Connections {
        function onViewChanged() {
            rig.load();
        }

        target: rig.page.editor
    }

    QQC2.ComboBox {
        Layout.fillWidth: true
        model: rig.input.field.options
        currentIndex: Math.max(0, rig.input.field.options.indexOf(rig.input.value))
        onActivated: index => rig.input.commit(rig.input.field.options[index])
    }

    QQC2.Button {
        text: App.tr("Place bones on the preview")
        icon.name: "transform-move"
        checkable: true
        enabled: rig.editable
        checked: rig.page.mesh.rigging === rig.layerId && rig.page.mesh.sculpt === null && rig.page.mesh.weighing === null && rig.page.mesh.mode === ""
        onToggled: rig.page.mesh.tool(rig.layerId, "bones", checked ? {} : null)
    }

    SceneBoneTree {
        Layout.fillWidth: true
        host: rig
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: !rig.editable
        wrapMode: Text.Wrap
        text: App.tr("Bones are placed here on meshes whose parts are each drawn once; other meshes keep the bones of their file.")
    }

    SceneRigWeights {
        Layout.fillWidth: true
        host: rig
    }

    SceneRigDepth {
        Layout.fillWidth: true
        page: rig.page
        layerId: rig.layerId
        editable: rig.editable
    }

    SceneShapes {
        Layout.fillWidth: true
        host: rig
    }

    SceneRigMotion {
        Layout.fillWidth: true
        host: rig
    }

    SceneRigWind {
        Layout.fillWidth: true
        host: rig
    }

    SceneRigAttach {
        Layout.fillWidth: true
        host: rig
    }

    SceneParts {
        Layout.fillWidth: true
        host: rig
    }

    RowLayout {
        QQC2.Label {
            text: App.tr("Clip in the preview")
        }

        QQC2.ComboBox {
            Layout.fillWidth: true
            model: [App.tr("None")].concat(rig.clips)
            onActivated: index => rig.page.run("set", {
                    "pointer": rig.base + "/animations",
                    "value": index === 0 ? [] : [
                        {
                            "clip": rig.clips[index - 1]
                        }
                    ]
                })
        }
    }

    QQC2.Button {
        text: App.tr("Edit clips on the timeline")
        icon.name: "chronometer"
        onClicked: {
            if (rig.clips.length === 0)
                rig.page.run("clip", {
                    "layer": rig.layerId,
                    "id": "clip_1",
                    "add": true
                });
            rig.page.mesh.layer = rig.layerId;
            rig.page.mesh.clip = rig.clips[0] ?? "clip_1";
        }
    }
}
