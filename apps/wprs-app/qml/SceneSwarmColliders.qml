import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: walls

    required property var tools
    readonly property var list: tools.info?.colliders ?? []
    readonly property var kinds: ["canvas", "plane", "circle", "box", "segment", "layer"]
    readonly property var names: [App.tr("Canvas"), App.tr("Plane"), App.tr("Circle"), App.tr("Box"), App.tr("Segment"), App.tr("Layer")]

    function fresh(kind) {
        const c = [960, 540];
        return {
            "canvas": "canvas",
            "plane": {
                "plane": {
                    "point": [960, 900],
                    "normal": [0, -1]
                }
            },
            "circle": {
                "circle": {
                    "center": c,
                    "radius": 200
                }
            },
            "box": {
                "box": {
                    "from": [760, 340],
                    "to": [1160, 740]
                }
            },
            "segment": {
                "segment": {
                    "from": [660, 700],
                    "to": [1260, 700]
                }
            },
            "layer": {
                "layer": {
                    "id": tools.info?.sources[0] ?? ""
                }
            }
        }[kind];
    }

    RowLayout {
        Kirigami.Heading {
            Layout.fillWidth: true
            level: 4
            text: App.tr("Colliders")
        }

        QQC2.ComboBox {
            model: walls.names
            displayText: App.tr("Add a collider")
            enabled: walls.list.length < 8
            onActivated: index => walls.tools.edit("collider", {
                        "collider": {
                            "bounds": walls.fresh(walls.kinds[index]),
                            "bounce": 0.5
                        }
                    })
        }
    }

    Repeater {
        model: walls.list

        delegate: SceneSwarmCollider {
            required property var modelData

            Layout.fillWidth: true
            tools: walls.tools
            wall: modelData
            names: walls.names
            kinds: walls.kinds
        }
    }
}
