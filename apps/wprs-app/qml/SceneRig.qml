import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: rig

    required property var input
    readonly property string base: input.field.pointer.replace(/\/[^/]*$/, "")
    readonly property string layerId: JSON.parse(input.page.editor.value(base + "/id") || "\"\"") ?? ""
    property var info: ({})
    readonly property var bones: info.bones ?? []
    readonly property var clips: info.clips ?? []

    function load() {
        info = JSON.parse(input.page.editor.rig(layerId) || "null") ?? {};
    }

    function bone(args) {
        input.page.run("bone", Object.assign({
            "layer": rig.layerId
        }, args));
    }

    Component.onCompleted: load()

    Connections {
        function onViewChanged() {
            rig.load();
        }

        target: rig.input.page.editor
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
        enabled: rig.info.editable === true
        checked: rig.input.page.rigging === rig.layerId
        onToggled: rig.input.page.rigging = checked ? rig.layerId : ""
    }

    Repeater {
        model: rig.bones

        delegate: RowLayout {
            required property var modelData

            QQC2.Label {
                Layout.fillWidth: true
                text: modelData.parent ? App.tr("%1, hanging from %2", modelData.name, modelData.parent) : modelData.name
            }

            QQC2.ToolButton {
                icon.name: "edit-delete"
                text: App.tr("Remove")
                display: QQC2.AbstractButton.IconOnly
                visible: rig.info.editable === true
                onClicked: rig.bone({
                    "name": modelData.name,
                    "remove": true
                })
            }
        }
    }

    RowLayout {
        visible: rig.info.editable === true

        QQC2.TextField {
            id: named

            Layout.fillWidth: true
            placeholderText: App.tr("Name of a new bone")
        }

        QQC2.ComboBox {
            id: under

            model: [App.tr("No parent")].concat(rig.bones.map(b => b.name))
        }

        QQC2.Button {
            text: App.tr("Add")
            enabled: named.text.trim() !== ""
            onClicked: {
                rig.bone({
                    "name": named.text.trim(),
                    "parent": under.currentIndex > 0 ? rig.bones[under.currentIndex - 1].name : null
                });
                named.text = "";
            }
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: rig.info.editable !== true
        wrapMode: Text.Wrap
        text: App.tr("Bones are placed here on meshes cut from a picture; other meshes keep the bones of their file.")
    }

    SceneRigMotion {
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
            onActivated: index => rig.input.page.run("set", {
                "pointer": rig.base + "/animations",
                "value": index === 0 ? [] : [{
                    "clip": rig.clips[index - 1]
                }]
            })
        }
    }
}
