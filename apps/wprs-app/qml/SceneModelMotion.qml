import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: motion

    required property var page
    required property string layerId
    property var info: null
    property string bone
    readonly property var files: info?.files ?? []
    readonly property var bones: (info?.bones ?? []).filter(b => !b.point).map(b => b.name)

    function load() {
        info = layerId ? JSON.parse(page.editor.rig("", JSON.stringify({
            "motion": {
                "layer": layerId
            }
        })) || "null") : null;
    }

    function animate(kind, args) {
        const out = {};
        out[kind] = Object.assign({
            "layer": layerId
        }, args);
        page.run("animate", out, true);
    }

    function fromFile(name) {
        const id = name.toLowerCase().replace(/[^a-z0-9_]/g, "_").slice(0, 32) || "clip";
        page.run("clip", {
            "layer": layerId,
            "id": id,
            "add": true
        }, true);
        page.run("clip", {
            "layer": layerId,
            "id": id,
            "animation": name
        }, true);
        edit(id);
    }

    function defaults() {
        const out = {};
        (info?.shapes ?? []).forEach((s, i) => out[s] = info.defaults[i] ?? 0);
        return out;
    }

    function edit(clip) {
        page.mesh.layer = layerId;
        page.mesh.clip = clip;
    }

    visible: info !== null && (files.length > 0 || bones.length > 0 || (info?.shapes ?? []).length > 0)
    onLayerIdChanged: load()
    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            motion.load();
        }

        target: motion.page.editor
    }

    Kirigami.Heading {
        level: 5
        text: App.tr("Animation")
    }

    Repeater {
        model: motion.info?.clips ?? []

        delegate: RowLayout {
            required property var modelData

            QQC2.Label {
                Layout.fillWidth: true
                text: App.tr("%1, %2 s from %3", modelData.id, modelData.length, modelData.animation ?? App.tr("its own keys"))
                elide: Text.ElideRight
            }

            QQC2.ToolButton {
                icon.name: "document-edit"
                text: App.tr("On the timeline")
                onClicked: motion.edit(modelData.id)
            }
        }
    }

    QQC2.ComboBox {
        Layout.fillWidth: true
        visible: motion.files.length > 0
        displayText: App.tr("New clip from the file…")
        model: motion.files.map(f => App.tr("%1 (%2 s)", f.name, f.length))
        onActivated: index => motion.fromFile(motion.files[index].name)
    }

    QQC2.CheckBox {
        text: App.tr("Root motion: the clip moves the layer")
        checked: motion.info?.root_motion === true
        enabled: (motion.info?.clips ?? []).length > 0
        onToggled: motion.animate("root", {
            "on": checked
        })
    }

    SceneModelSwing {
        Layout.fillWidth: true
        motion: motion
    }

    SceneSliders {
        Layout.fillWidth: true
        visible: (motion.info?.shapes ?? []).length > 0
        value: Object.assign(motion.defaults(), motion.info?.weights ?? {})
        rows: (motion.info?.shapes ?? []).map(s => [s, App.tr("Shape %1", s), -1, 1, 0.01])
        onEdited: (key, value) => motion.animate("morph", {
                "shape": key,
                "weight": value
            })
    }

    SceneAnimLayers {
        Layout.fillWidth: true
        page: motion.page
        layerId: motion.layerId
    }
}
