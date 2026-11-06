import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: swing

    required property var motion
    readonly property var presets: motion.info?.presets ?? []
    readonly property var bodies: motion.info?.physics ?? []
    readonly property var modes: ["spring", "rigid", "rope"]
    readonly property var knobs: [["stiffness", 0, 1, 0.5], ["damping", 0, 1, 0.5], ["inertia", 0, 1, 0.5], ["gravity", -10, 10, 0], ["wind", 0, 1, 0]]
    readonly property var words: ({
            "hair": App.tr("Hair"),
            "tail": App.tr("Tail"),
            "cloth": App.tr("Cloth"),
            "antenna": App.tr("Antenna"),
            "chain": App.tr("Chain"),
            "jelly": App.tr("Jelly")
        })

    function run(bone, args, gesture) {
        motion.page.run("physics", Object.assign({
                    "layer": motion.layerId,
                    "bones": [bone]
                }, args), gesture);
    }

    visible: motion.bones.length > 0

    QQC2.Label {
        text: App.tr("Swinging bones")
        font.bold: true
    }

    SceneBonePick {
        Layout.fillWidth: true
        names: swing.motion.bones
        chosen: swing.motion.bone !== "" ? [swing.motion.bone] : []
        single: true
        onPicked: list => {
            swing.motion.bone = list[0] ?? "";
            swing.motion.page.mesh.bone = swing.motion.bone;
        }
    }

    Flow {
        Layout.fillWidth: true
        enabled: swing.motion.bone !== ""

        Repeater {
            model: swing.presets

            delegate: QQC2.Button {
                required property string modelData

                text: swing.words[modelData] ?? modelData
                onClicked: swing.motion.animate("preset", {
                        "bone": swing.motion.bone,
                        "preset": modelData
                    })
            }
        }
    }

    Repeater {
        model: swing.bodies

        delegate: ColumnLayout {
            id: body

            required property var modelData

            Layout.fillWidth: true

            RowLayout {
                QQC2.Label {
                    Layout.fillWidth: true
                    text: body.modelData.bone
                    font.italic: true
                }

                QQC2.ComboBox {
                    model: swing.modes.map(m => SceneWords.value(m))
                    currentIndex: Math.max(0, swing.modes.indexOf(body.modelData.mode ?? "spring"))
                    onActivated: index => swing.run(body.modelData.bone, {
                                "mode": swing.modes[index]
                            })
                }

                QQC2.ToolButton {
                    icon.name: "edit-delete"
                    text: App.tr("Remove")
                    display: QQC2.AbstractButton.IconOnly
                    onClicked: swing.run(body.modelData.bone, {
                            "remove": true
                        })
                }
            }

            Repeater {
                model: swing.knobs

                delegate: RowLayout {
                    required property var modelData

                    QQC2.Label {
                        text: SceneWords.label(modelData[0], "")
                    }

                    SceneSlider {
                        Layout.fillWidth: true
                        from: modelData[1]
                        to: modelData[2]
                        value: body.modelData[modelData[0]] ?? modelData[3]
                        onMoved: {
                            const args = {};
                            args[modelData[0]] = Math.round(value * 100) / 100;
                            swing.run(body.modelData.bone, args, true);
                        }
                    }
                }
            }

            ScenePhysicsBounds {
                Layout.fillWidth: true
                owner: swing
                body: body.modelData
            }
        }
    }

    QQC2.Button {
        text: App.tr("Show the wind on the preview")
        icon.name: "weather-windy"
        visible: swing.bodies.length > 0
        checkable: true
        checked: swing.motion.page.mesh.mode === "wind"
        onToggled: swing.motion.page.mesh.use(swing.motion.layerId, checked ? "wind" : "")
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: swing.bodies.length > 0
        wrapMode: Text.Wrap
        font: Kirigami.Theme.smallFont
        text: App.tr("Swinging stops at the lowest quality level, where the bones keep their animation.")
    }
}
