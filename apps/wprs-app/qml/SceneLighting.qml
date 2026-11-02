import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: lit

    required property var page
    required property var node
    readonly property bool takes: ["image", "video", "text"].includes(node?.type ?? "")
    readonly property var block: takes ? JSON.parse(page.editor.value(node.pointer + "/lighting") || "null") : null
    readonly property var pictures: [""].concat((page.view.assets ?? []).filter(a => a.kind === "image").map(a => a.id))
    readonly property var fields: [["normal", App.tr("Normal map")], ["env", App.tr("Reflected picture")], ["roughness_map", App.tr("Roughness map")], ["emissive_map", App.tr("Glow map")]]

    function edit(args) {
        page.run("lighting", Object.assign({
            "layer": node.id
        }, args));
    }

    visible: takes

    Kirigami.Heading {
        level: 4
        text: App.tr("Lighting")
    }

    RowLayout {
        QQC2.Switch {
            text: App.tr("Take the light of light layers")
            checked: lit.block !== null
            onToggled: lit.edit(checked ? {} : {
                "off": true
            })
        }

        QQC2.Button {
            visible: lit.block !== null && lit.node?.type === "image"
            text: App.tr("Generate normals…")
            icon.name: "draw-cuboid"
            onClicked: normals.open()
        }
    }

    GridLayout {
        visible: lit.block !== null
        columns: 2

        Repeater {
            model: lit.fields

            delegate: QQC2.Label {
                required property var modelData
                required property int index

                Layout.row: index
                Layout.column: 0
                text: modelData[1]
            }
        }

        Repeater {
            model: lit.fields

            delegate: QQC2.ComboBox {
                required property var modelData
                required property int index

                Layout.row: index
                Layout.column: 1
                Layout.fillWidth: true
                model: lit.pictures.map(p => p || App.tr("Not set"))
                currentIndex: Math.max(0, lit.pictures.indexOf(lit.block?.[modelData[0]] ?? ""))
                onActivated: i => lit.edit({
                        [modelData[0]]: lit.pictures[i]
                    })
            }
        }
    }

    SceneSliders {
        Layout.fillWidth: true
        visible: lit.block !== null
        value: lit.block ?? {}
        rows: [["roughness", App.tr("Roughness"), 0, 1, 0.01], ["reflection", App.tr("Reflection"), 0, 1, 0.01], ["emissive", App.tr("Glow"), 0, 16, 0.05], ["strength", App.tr("Strength"), 0, 1, 0.01]]
        onEdited: (key, value) => lit.page.change(lit.node.pointer + "/lighting/" + key, value, true)
    }

    SceneMapBrush {
        Layout.fillWidth: true
        visible: lit.block !== null
        page: lit.page
        node: lit.node
    }

    SceneNormals {
        id: normals

        page: lit.page
        node: lit.node
    }
}
