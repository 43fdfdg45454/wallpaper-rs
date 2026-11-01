import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

ColumnLayout {
    id: marks

    required property var host
    readonly property var page: host.page
    readonly property var points: (host.info.points ?? []).filter(p => !p.part)
    readonly property var parents: [""].concat(host.bones.map(b => b.name))

    function run(name, args) {
        page.run("point", Object.assign({
            "layer": host.layerId,
            "name": name
        }, args));
    }

    function fresh() {
        const taken = (host.info.points ?? []).concat(host.bones).map(p => p.name);
        let name = "point";
        for (let i = 2; taken.includes(name); i++)
            name = "point_" + i;
        return name;
    }

    QQC2.Label {
        text: App.tr("Attachment points")
        font.bold: true
    }

    Repeater {
        model: marks.points

        delegate: RowLayout {
            id: row

            required property var modelData

            Layout.fillWidth: true

            QQC2.TextField {
                Layout.fillWidth: true
                text: row.modelData.name
                enabled: marks.host.editable
                onEditingFinished: {
                    if (text !== "" && text !== row.modelData.name)
                        marks.run(row.modelData.name, {
                            "to": text
                        });
                }
            }

            QQC2.ComboBox {
                enabled: marks.host.editable
                model: marks.parents.map(p => p === "" ? App.tr("No bone") : p)
                currentIndex: Math.max(0, marks.parents.indexOf(row.modelData.parent ?? ""))
                onActivated: index => marks.run(row.modelData.name, {
                        "parent": marks.parents[index]
                    })
            }

            QQC2.ToolButton {
                icon.name: "edit-delete"
                text: App.tr("Remove the point")
                display: QQC2.AbstractButton.IconOnly
                enabled: marks.host.editable
                onClicked: marks.run(row.modelData.name, {
                    "remove": true
                })
            }
        }
    }

    QQC2.Button {
        text: App.tr("Add a point")
        icon.name: "list-add"
        enabled: marks.host.editable
        onClicked: {
            const name = marks.fresh();
            marks.run(name, {
                "at": [0.5, 0.5]
            });
            marks.page.mesh.spot = name;
            marks.page.mesh.use(marks.host.layerId, "attach");
        }
    }
}
