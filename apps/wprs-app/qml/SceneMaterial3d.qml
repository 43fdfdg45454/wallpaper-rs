import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

GridLayout {
    id: row

    required property var panel
    required property var material
    readonly property var set: material.set ?? {}
    readonly property var file: material.file ?? {}
    readonly property var shaders: panel.info?.shaders ?? ["pbr", "unlit"]
    readonly property var names: ({
            "base": App.tr("Color"),
            "orm": App.tr("Metal and roughness"),
            "normal": App.tr("Normals"),
            "occlusion": App.tr("Occlusion"),
            "emissive": App.tr("Glow")
        })

    function hex(c) {
        return "#" + c.slice(0, 3).map(v => Math.round(Math.pow(Math.min(Math.max(v, 0), 1), 1 / 2.2) * 255).toString(16).padStart(2, "0")).join("");
    }

    columns: 2

    Image {
        Layout.rowSpan: 4
        source: row.material.sphere ?? ""
        cache: false
    }

    RowLayout {
        Layout.fillWidth: true

        QQC2.Label {
            text: row.material.name
            font.bold: true
        }

        QQC2.ComboBox {
            Layout.fillWidth: true
            model: row.shaders.map(s => s === "pbr" ? App.tr("Physical") : s === "unlit" ? App.tr("Unlit") : s)
            currentIndex: Math.max(0, row.shaders.indexOf(row.set.shader ?? (row.file.unlit ? "unlit" : "pbr")))
            onActivated: index => row.panel.set(row.material.name, {
                    "shader": row.shaders[index]
                })
        }
    }

    Flow {
        Layout.fillWidth: true

        Repeater {
            model: [["lit", App.tr("Lit"), true], ["double_sided", App.tr("Both sides"), row.file.double_sided === true], ["cast_shadows", App.tr("Casts shadows"), true], ["receive_shadows", App.tr("Takes shadows"), true]]

            delegate: QQC2.CheckBox {
                required property var modelData

                text: modelData[1]
                checked: row.set[modelData[0]] ?? modelData[2]
                onToggled: {
                    const args = {};
                    args[modelData[0]] = checked;
                    row.panel.set(row.material.name, args);
                }
            }
        }
    }

    RowLayout {
        QQC2.Label {
            text: App.tr("Tint")
        }

        SceneColor {
            alpha: false
            hex: typeof row.set.color === "string" ? row.set.color : "#ffffff"
            onPicked: hex => row.panel.set(row.material.name, {
                    "color": hex
                })
        }

        QQC2.Label {
            text: App.tr("Glow")
        }

        SceneColor {
            alpha: false
            hex: typeof row.set.emissive === "string" ? row.set.emissive : row.hex(row.file.emissive ?? [0, 0, 0])
            onPicked: hex => row.panel.set(row.material.name, {
                    "emissive": hex
                })
        }
    }

    SceneSliders {
        Layout.fillWidth: true
        value: Object.assign({
            "metallic": row.file.metallic ?? 1,
            "roughness": row.file.roughness ?? 1,
            "normal_scale": row.file.normal_scale ?? 1,
            "occlusion": row.file.occlusion ?? 1
        }, row.set.params ?? {})
        rows: [["metallic", App.tr("Metal"), 0, 1, 0.01], ["roughness", App.tr("Roughness"), 0, 1, 0.01], ["normal_scale", App.tr("Relief"), 0, 10, 0.05], ["occlusion", App.tr("Occlusion"), 0, 1, 0.01]]
        onEdited: (key, value) => {
            const params = {};
            params[key] = value;
            row.panel.set(row.material.name, {
                "params": params
            });
        }
    }

    Flow {
        Layout.columnSpan: 2
        Layout.fillWidth: true

        Repeater {
            model: row.material.maps ?? []

            delegate: ColumnLayout {
                required property var modelData

                Image {
                    Layout.preferredWidth: Kirigami.Units.gridUnit * 2.5
                    Layout.preferredHeight: Layout.preferredWidth
                    source: modelData.thumb ?? ""
                    visible: modelData.thumb !== null
                }

                QQC2.Label {
                    text: row.names[modelData.slot] + (modelData.thumb === null ? " " + App.tr("(none)") : "")
                    font: Kirigami.Theme.smallFont
                }
            }
        }
    }
}
