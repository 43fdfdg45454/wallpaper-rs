import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: shapes

    required property var host
    readonly property var mesh: host.page.mesh
    readonly property var names: host.info.shapes ?? []
    readonly property var expressions: host.info.expressions ?? {}
    readonly property var weights: mesh.layer === host.layerId ? mesh.shapes : ({})
    property string editing

    function weigh(name, value) {
        const next = Object.assign({}, weights);
        next[name] = Math.round(value * 100) / 100;
        mesh.layer = host.layerId;
        mesh.shapes = next;
    }

    function save(name) {
        const set = {};
        names.filter(n => (weights[n] ?? 0) !== 0).forEach(n => set[n] = weights[n]);
        host.page.run("expression", {
            "layer": host.layerId,
            "name": name,
            "shapes": set
        });
    }

    visible: names.length > 0

    Kirigami.Separator {
        Layout.fillWidth: true
    }

    QQC2.Label {
        text: App.tr("Blend shapes in the preview")
        font.bold: true
    }

    Repeater {
        model: shapes.names

        delegate: RowLayout {
            required property string modelData

            QQC2.Label {
                Layout.preferredWidth: Kirigami.Units.gridUnit * 5
                text: modelData
                elide: Text.ElideRight
            }

            QQC2.Slider {
                Layout.fillWidth: true
                from: -1
                to: 1
                value: shapes.weights[modelData] ?? 0
                onMoved: shapes.weigh(modelData, value)
            }
        }
    }

    QQC2.Button {
        text: App.tr("Reset the preview")
        visible: Object.keys(shapes.weights).length > 0
        onClicked: shapes.mesh.shapes = {}
    }

    QQC2.Label {
        text: App.tr("Expressions")
        font.bold: true
    }

    Repeater {
        model: Object.keys(shapes.expressions)

        delegate: RowLayout {
            required property string modelData

            QQC2.Label {
                Layout.fillWidth: true
                text: modelData
                font.bold: shapes.editing === modelData
                elide: Text.ElideRight
            }

            QQC2.ToolButton {
                icon.name: "document-edit"
                text: App.tr("Show in the preview to edit")
                display: QQC2.AbstractButton.IconOnly
                onClicked: {
                    shapes.editing = modelData;
                    shapes.mesh.layer = shapes.host.layerId;
                    shapes.mesh.shapes = Object.assign({}, shapes.expressions[modelData]);
                }
            }

            QQC2.ToolButton {
                icon.name: "document-save"
                text: App.tr("Keep the sliders in this expression")
                display: QQC2.AbstractButton.IconOnly
                onClicked: shapes.save(modelData)
            }

            QQC2.ToolButton {
                icon.name: "edit-delete"
                text: App.tr("Remove")
                display: QQC2.AbstractButton.IconOnly
                onClicked: shapes.host.page.run("expression", {
                    "layer": shapes.host.layerId,
                    "name": modelData,
                    "remove": true
                })
            }
        }
    }

    RowLayout {
        QQC2.TextField {
            id: named

            Layout.fillWidth: true
            placeholderText: App.tr("Name of a new expression")
        }

        QQC2.Button {
            text: App.tr("Save the sliders")
            enabled: named.text.trim() !== ""
            onClicked: {
                shapes.save(named.text.trim());
                named.text = "";
            }
        }
    }

    RowLayout {
        visible: shapes.editing !== "" && shapes.expressions[shapes.editing] !== undefined

        QQC2.TextField {
            id: renamed

            Layout.fillWidth: true
            placeholderText: App.tr("New name for %1", shapes.editing)
        }

        QQC2.Button {
            text: App.tr("Rename")
            enabled: renamed.text.trim() !== ""
            onClicked: {
                shapes.host.page.run("expression", {
                    "layer": shapes.host.layerId,
                    "name": shapes.editing,
                    "to": renamed.text.trim()
                });
                shapes.editing = renamed.text.trim();
                renamed.text = "";
            }
        }
    }
}
