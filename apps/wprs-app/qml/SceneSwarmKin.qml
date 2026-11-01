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

    Repeater {
        model: kin.kids

        delegate: ColumnLayout {
            id: child

            required property var modelData

            QQC2.Label {
                text: App.tr("Child %1 takes from its parent:", child.modelData.system)
            }

            Flow {
                Layout.fillWidth: true

                Repeater {
                    model: kin.takes

                    delegate: QQC2.CheckBox {
                        required property string modelData

                        text: ({
                                "color": App.tr("Color"),
                                "size": App.tr("Size"),
                                "alpha": App.tr("Alpha"),
                                "velocity": App.tr("Velocity")
                            })[modelData]
                        checked: child.modelData.inherit.includes(modelData)
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
