import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

ColumnLayout {
    id: swing

    required property var owner
    readonly property var page: owner.host.input.page
    readonly property var mesh: page.mesh
    readonly property string layerId: owner.host.layerId
    readonly property var modes: ["spring", "rigid", "rope"]
    readonly property var knobs: [["stiffness", 0, 1, 0.5], ["damping", 0, 1, 0.5], ["inertia", 0, 1, 0.5], ["gravity", -10, 10, 0], ["wind", 0, 1, 0]]

    function run(bone, args, gesture) {
        page.run("physics", Object.assign({
            "layer": layerId,
            "bones": [bone]
        }, args), gesture);
    }

    function use(kind, on) {
        mesh.use(layerId, on ? kind : "");
    }

    QQC2.Label {
        text: App.tr("Bones that swing")
        font.bold: true
    }

    Repeater {
        model: swing.owner.bodies

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
                    currentIndex: swing.modes.indexOf(body.modelData.mode ?? "spring")
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

            GridLayout {
                columns: 2

                Repeater {
                    model: swing.knobs

                    delegate: RowLayout {
                        required property var modelData

                        Layout.columnSpan: 2

                        QQC2.Label {
                            Layout.preferredWidth: implicitWidth
                            text: SceneWords.label(modelData[0], "")
                        }

                        QQC2.Slider {
                            Layout.fillWidth: true
                            from: modelData[1]
                            to: modelData[2]
                            value: body.modelData[modelData[0]] ?? modelData[3]
                            onMoved: swing.run(body.modelData.bone, swing.owner.pair(modelData[0], Math.round(value * 100) / 100), true)
                        }
                    }
                }
            }
        }
    }

    QQC2.Button {
        text: App.tr("Pick bones that swing on the preview")
        icon.name: "edit-select"
        checkable: true
        checked: swing.mesh.mode === "bodies" && swing.mesh.rigging === swing.layerId
        onToggled: swing.use("bodies", checked)
    }

    QQC2.Button {
        text: App.tr("Draw a rope on the preview")
        icon.name: "draw-freehand"
        checkable: true
        checked: swing.mesh.mode === "rope" && swing.mesh.rigging === swing.layerId
        onToggled: swing.use("rope", checked)
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: swing.mesh.mode === "rope" || swing.mesh.mode === "bodies"
        wrapMode: Text.Wrap
        text: swing.mesh.mode === "rope" ? App.tr("Drag along where the rope hangs: it gets up to 9 new bones and hangs from the picked bone, or the nearest one.") : App.tr("Click a bone to make it swing, or to stop it. The arrows show gravity.")
    }

    RowLayout {
        QQC2.Switch {
            text: App.tr("Simulate")
            checked: !swing.mesh.still
            onToggled: {
                swing.mesh.layer = swing.layerId;
                swing.mesh.still = !checked;
                if (checked)
                    swing.page.editor.seek(swing.page.clock, true);
            }
        }

        QQC2.Button {
            text: App.tr("Reset")
            icon.name: "view-refresh"
            onClicked: {
                swing.mesh.layer = swing.layerId;
                swing.mesh.reset();
            }
        }
    }
}
