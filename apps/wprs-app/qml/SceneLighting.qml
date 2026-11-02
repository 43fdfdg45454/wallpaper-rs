import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: lit

    required property var page
    required property var node
    readonly property bool takes: ["image", "video", "text", "particles"].includes(node?.type ?? "")
    readonly property bool sprites: node?.type === "particles"
    readonly property var block: takes ? JSON.parse(page.editor.value(node.pointer + "/lighting") || "null") : null
    readonly property var pictures: [""].concat((page.view.assets ?? []).filter(a => a.kind === "image").map(a => a.id))
    readonly property var fields: [["normal", App.tr("Normal map")], ["env", App.tr("Reflected picture")], ["roughness_map", App.tr("Roughness map")], ["emissive_map", App.tr("Glow map")]]
    readonly property var colored: block ? ["normal", "roughness_map", "emissive_map"].filter(k => block[k] && page.editor.value("/scene/assets/" + String(block[k]).replace(/~/g, "~0").replace(/\//g, "~1") + "/linear") !== "true") : []

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
        visible: lit.block !== null && !lit.sprites
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

    QQC2.Label {
        Layout.fillWidth: true
        visible: lit.colored.length > 0 && !lit.sprites
        wrapMode: Text.Wrap
        color: Kirigami.Theme.neutralTextColor
        text: App.tr("%1 is read as colors and shades wrong; pick it again to read it as a map.", lit.colored.map(k => lit.block[k]).join(", "))
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: lit.block !== null && lit.sprites
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        text: App.tr("Particles take the strength and the glow here; their normal map and refraction are in the particle system.")
    }

    SceneSliders {
        Layout.fillWidth: true
        visible: lit.block !== null
        value: lit.block ?? {}
        rows: lit.sprites ? [["emissive", App.tr("Glow"), 0, 16, 0.05], ["strength", App.tr("Strength"), 0, 1, 0.01]] : [["roughness", App.tr("Roughness"), 0, 1, 0.01], ["reflection", App.tr("Reflection"), 0, 1, 0.01], ["emissive", App.tr("Glow"), 0, 16, 0.05], ["strength", App.tr("Strength"), 0, 1, 0.01]]
        onEdited: (key, value) => lit.page.change(lit.node.pointer + "/lighting/" + key, value, true)
    }

    SceneMapBrush {
        Layout.fillWidth: true
        visible: lit.block !== null && !lit.sprites
        page: lit.page
        node: lit.node
    }

    SceneNormals {
        id: normals

        page: lit.page
        node: lit.node
    }
}
