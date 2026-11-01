import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: row

    required property var tools
    required property var emitter
    readonly property var kinds: ["point", "box", "circle", "line", "edge", "around", "between", "layer"]
    readonly property var targets: (tools.info?.points ?? []).map(p => p.name).concat(["cursor"])

    function fresh(kind) {
        const c = [960, 540];
        return {
            "point": {
                "point": c
            },
            "box": {
                "box": [[860, 440], [1060, 640]]
            },
            "circle": {
                "circle": {
                    "center": c,
                    "radius": 100
                }
            },
            "line": {
                "line": [[760, 540], [1160, 540]]
            },
            "edge": "edge",
            "around": {
                "around": {
                    "at": "p0",
                    "count": 24,
                    "radius": 120
                }
            },
            "between": {
                "between": {
                    "from": "p0",
                    "to": c,
                    "count": 16
                }
            },
            "layer": {
                "layer": {
                    "id": tools.info?.sources?.[0] ?? ""
                }
            }
        }[kind];
    }

    function patch(change) {
        const s = tools.page.editor.value(SceneSwarmKnobs.pointer(tools.system, "/emitters/" + emitter.index + "/shape"));
        const shape = JSON.parse(s || "{}");
        change(shape[emitter.kind]);
        tools.edit("shape", {
            "emitter": emitter.index,
            "shape": shape
        });
    }

    RowLayout {
        QQC2.Label {
            text: App.tr("Emitter %1", row.emitter.index + 1)
        }

        QQC2.ComboBox {
            Layout.fillWidth: true
            model: [App.tr("Point"), App.tr("Box"), App.tr("Circle"), App.tr("Line"), App.tr("Edge"), App.tr("Around a point"), App.tr("Between two points"), App.tr("From a layer")]
            currentIndex: row.kinds.indexOf(row.emitter.kind)
            onActivated: index => tools.edit("shape", {
                    "emitter": row.emitter.index,
                    "shape": row.fresh(row.kinds[index])
                })
        }
    }

    RowLayout {
        visible: row.emitter.kind === "around" || row.emitter.kind === "between"

        Repeater {
            model: row.emitter.kind === "around" ? ["at"] : ["from_ref", "to_ref"]

            delegate: QQC2.ComboBox {
                required property string modelData
                readonly property var value: row.emitter[modelData === "at" ? "at" : modelData]

                Layout.fillWidth: true
                model: row.targets.concat(Array.isArray(value) ? [App.tr("a point")] : [])
                currentIndex: Array.isArray(value) ? row.targets.length : row.targets.indexOf(value)
                onActivated: index => {
                    if (index < row.targets.length)
                        row.patch(s => s[modelData === "at" ? "at" : modelData.replace("_ref", "")] = row.targets[index]);
                }
            }
        }

        QQC2.SpinBox {
            from: 1
            to: 4096
            value: row.emitter.count ?? 1
            onValueModified: row.patch(s => s.count = value)
        }
    }

    SceneSwarmSource {
        Layout.fillWidth: true
        visible: row.emitter.kind === "layer"
        tools: row.tools
        emitter: row.emitter
    }
}
