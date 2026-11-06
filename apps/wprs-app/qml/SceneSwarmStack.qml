import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: stack

    required property var tools
    readonly property var system: (tools.info, JSON.parse(tools.page.editor.value(SceneSwarmKnobs.pointer(tools.system, "")) || "null")) ?? ({})
    readonly property var render: system.render ?? ({})
    readonly property var orders: ["none", "newest", "oldest", "largest", "smallest", "lowest"]
    readonly property var tunables: ["alpha", "size", "count", "speed", "life", "time_scale", "brightness", "color", "points"]
    readonly property var locked: system.lock ?? []

    Kirigami.Heading {
        level: 4
        text: App.tr("Drawing order and what layers may change")
    }

    RowLayout {
        QQC2.Label {
            text: App.tr("On top")
        }

        QQC2.ComboBox {
            Layout.fillWidth: true
            model: [App.tr("In no set order"), App.tr("The newest"), App.tr("The oldest"), App.tr("The largest"), App.tr("The smallest"), App.tr("The lowest")]
            currentIndex: Math.max(0, stack.orders.indexOf(stack.render.order ?? "none"))
            onActivated: index => stack.tools.edit("render", {
                        "order": stack.orders[index]
                    })
        }
    }

    RowLayout {
        QQC2.Label {
            text: App.tr("Times the frames repeat in a life, with frames over the life; 0 once")
        }

        QQC2.SpinBox {
            from: 0
            to: 64
            editable: true
            value: stack.render.cycles ?? 0
            onValueModified: stack.tools.edit("render", {
                    "cycles": value
                })
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        wrapMode: Text.Wrap
        text: App.tr("Particle layers may change these values of the system; untick one to lock it:")
    }

    Flow {
        Layout.fillWidth: true
        spacing: Kirigami.Units.smallSpacing

        SceneRepeater {
            id: listed

            rows: stack.tunables

            delegate: QQC2.CheckBox {
                required property int index
                readonly property var modelData: listed.rows[index]

                text: SceneWords.label(modelData)
                checked: !stack.locked.includes(modelData)
                onToggled: {
                    const next = checked ? stack.locked.filter(k => k !== modelData) : stack.locked.concat([modelData]);
                    stack.tools.edit("settings", {
                            "lock": next.length ? next : ["none"]
                        });
                }
            }
        }
    }
}
