import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: kin

    required property var tools
    readonly property var kids: tools.info?.children ?? []
    readonly property var shaders: tools.info?.shaders ?? ({})
    readonly property var offered: [""].concat(shaders.offered ?? [])
    readonly property var takes: ["color", "size", "alpha", "velocity"]
    readonly property var moments: ["static", "spawn", "death", "follow"]

    function fresh() {
        const taken = Object.keys(JSON.parse(tools.page.editor.value("/scene/particles") || "{}"));
        const base = tools.system.slice(0, 24) + "_child";
        let n = 1;
        while (taken.includes(base + "_" + n))
            n++;
        return base + "_" + n;
    }

    SceneRepeater {
        id: listed2

        rows: kin.kids

        delegate: ColumnLayout {
            id: child

            required property int index
            readonly property var modelData: listed2.rows[index]

            RowLayout {
                Layout.fillWidth: true

                QQC2.Label {
                    Layout.fillWidth: true
                    text: App.tr("Child %1 takes from its parent:", child.modelData.system)
                }

                QQC2.ToolButton {
                    icon.name: "edit-delete"
                    text: App.tr("Remove the child")
                    display: QQC2.AbstractButton.IconOnly
                    onClicked: kin.tools.edit("child", {
                        "index": child.modelData.index,
                        "remove": true
                    })
                }
            }

            Flow {
                Layout.fillWidth: true

                SceneRepeater {
                    id: listed

                    rows: kin.takes

                    delegate: QQC2.CheckBox {
                        required property int index
                        readonly property var modelData: listed.rows[index]

                        text: ({
                                "color": App.tr("Color"),
                                "size": App.tr("Size"),
                                "alpha": App.tr("Alpha"),
                                "velocity": App.tr("Velocity")
                            })[modelData]
                        checked: child.modelData.inherit.includes(modelData)
                        QQC2.ToolTip.visible: hovered && modelData === "velocity"
                        QQC2.ToolTip.text: App.tr("Also the speed in depth, when the parent and the child both have perspective")
                        onToggled: kin.tools.edit("inherit", {
                            "child": child.modelData.index,
                            "follow": child.modelData.follow_inherit,
                            "inherit": kin.takes.filter(t => t === modelData ? checked : child.modelData.inherit.includes(t))
                        })
                    }
                }
            }

            QQC2.CheckBox {
                visible: child.modelData.on === "follow"
                text: App.tr("Every step while it follows")
                checked: child.modelData.follow_inherit
                onToggled: kin.tools.edit("inherit", {
                    "child": child.modelData.index,
                    "inherit": child.modelData.inherit,
                    "follow": checked
                })
            }
        }
    }

    RowLayout {
        Layout.fillWidth: true

        QQC2.ComboBox {
            id: moment

            Layout.fillWidth: true
            model: [App.tr("All the time"), App.tr("When a particle starts"), App.tr("When a particle ends"), App.tr("Following each particle")]
        }

        QQC2.Button {
            icon.name: "list-add"
            text: App.tr("New child system")
            onClicked: kin.tools.page.run("particles", {
                "create": {
                    "system": kin.fresh(),
                    "parent": kin.tools.system,
                    "on": kin.moments[moment.currentIndex]
                }
            })
        }
    }

    QQC2.Label {
        text: App.tr("Particle shader")
    }

    QQC2.ComboBox {
        Layout.fillWidth: true
        model: kin.offered.map(id => id || App.tr("None"))
        currentIndex: Math.max(0, kin.offered.indexOf(kin.shaders.current ?? ""))
        onActivated: index => kin.tools.edit("shader", index ? {
                "shader": kin.offered[index]
            } : {})
    }

    Kirigami.InlineMessage {
        Layout.fillWidth: true
        visible: kin.shaders.warning === true
        type: Kirigami.MessageType.Warning
        text: App.tr("Particle shaders come with the package. The engine checks them like other package effects and leaves out one it cannot build or that is too slow; the particles are then drawn without it.")
    }
}
