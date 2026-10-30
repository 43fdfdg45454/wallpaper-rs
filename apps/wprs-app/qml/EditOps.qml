import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import org.kde.kirigami as Kirigami
import io.github.wallpaper_rs.app

ColumnLayout {
    id: list

    required property var page
    readonly property var masks: Object.keys(page.edit.masks ?? {})
    readonly property var names: ({
            "exposure": App.tr("Exposure"),
            "white_balance": App.tr("White balance"),
            "contrast": App.tr("Contrast"),
            "saturation": App.tr("Saturation"),
            "curves": App.tr("Curves"),
            "blur": App.tr("Blur"),
            "overlay": App.tr("Color overlay"),
            "vignette": App.tr("Vignette")
        })

    function move(index, step) {
        const ids = page.ops.map(op => op.id);
        ids.splice(index + step, 0, ids.splice(index, 1)[0]);
        page.run("order", {
            "ids": ids
        });
    }

    function set(id, key, value) {
        page.run("set", {
            "changes": ["ops." + id + "." + key + "=" + value]
        });
    }

    Kirigami.Heading {
        level: 4
        text: App.tr("Operations, in order")
    }

    QQC2.Label {
        visible: list.page.ops.length === 0
        text: App.tr("No changes yet")
    }

    Repeater {
        model: list.page.ops.length

        delegate: ColumnLayout {
            required property int index
            readonly property var op: list.page.ops[index] ?? {}

            Layout.fillWidth: true

            RowLayout {
                QQC2.CheckBox {
                    Layout.fillWidth: true
                    text: list.names[op.type] ?? op.type
                    checked: op.enabled
                    onToggled: list.set(op.id, "enabled", checked)
                }

                QQC2.ToolButton {
                    text: App.tr("Move up")
                    display: QQC2.AbstractButton.IconOnly
                    icon.name: "go-up"
                    enabled: index > 0
                    onClicked: list.move(index, -1)
                }

                QQC2.ToolButton {
                    text: App.tr("Move down")
                    display: QQC2.AbstractButton.IconOnly
                    icon.name: "go-down"
                    enabled: index < list.page.ops.length - 1
                    onClicked: list.move(index, 1)
                }

                QQC2.ToolButton {
                    text: App.tr("Remove")
                    display: QQC2.AbstractButton.IconOnly
                    icon.name: "edit-delete"
                    onClicked: list.page.run("reset", {
                        "ids": [op.id]
                    })
                }
            }

            EditSlider {
                text: App.tr("Strength")
                value: op.opacity ?? 1
                onMoved: value => list.set(op.id, "opacity", value)
            }

            QQC2.ComboBox {
                Layout.fillWidth: true
                model: [App.tr("Whole picture")].concat(list.masks.map(name => App.tr("Mask %1", name)))
                currentIndex: list.masks.indexOf(op.mask ?? "") + 1
                onActivated: index => list.set(op.id, "mask", index > 0 ? list.masks[index - 1] : "null")
            }
        }
    }
}
