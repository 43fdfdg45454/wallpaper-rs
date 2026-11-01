import QtQuick
import QtQuick.Layouts
import QtQuick.Controls as QQC2
import io.github.wallpaper_rs.app

ColumnLayout {
    id: mask

    required property var input
    readonly property int channels: input.field.sample?.channels ?? 1
    readonly property bool painting: input.page.brush?.pointer === input.field.pointer
    readonly property var options: [null].concat(input.field.options.filter(option => typeof option === "string"))

    function brush() {
        input.page.brush = {
            "pointer": input.field.pointer,
            "channel": mask.channels > 1 ? channel.currentIndex : null,
            "radius": size.value,
            "hardness": 0.5,
            "value": erase.checked ? 0 : 1
        };
    }

    QQC2.ComboBox {
        Layout.fillWidth: true
        model: mask.options.map(option => option === null ? App.tr("Not set") : mask.input.name(option))
        currentIndex: Math.max(0, mask.options.indexOf(mask.input.value))
        onActivated: index => {
            if (index === 0)
                mask.input.page.run("remove", {
                    "pointer": mask.input.field.pointer
                });
            else
                mask.input.commit(mask.options[index]);
        }
    }

    RowLayout {
        QQC2.Button {
            text: App.tr("Paint")
            icon.name: "draw-brush"
            checkable: true
            checked: mask.painting
            onToggled: {
                if (checked)
                    mask.brush();
                else
                    mask.input.page.brush = null;
            }
        }

        QQC2.ComboBox {
            id: channel

            visible: mask.channels > 1
            model: [App.tr("Red"), App.tr("Green"), App.tr("Blue"), App.tr("Alpha")].slice(0, mask.channels)
            onActivated: {
                if (mask.painting)
                    mask.brush();
            }
        }

        QQC2.CheckBox {
            id: erase

            text: App.tr("Erase")
            onToggled: {
                if (mask.painting)
                    mask.brush();
            }
        }
    }

    RowLayout {
        visible: mask.painting

        QQC2.Label {
            text: App.tr("Brush size")
        }

        QQC2.Slider {
            id: size

            Layout.fillWidth: true
            from: 0.005
            to: 0.25
            value: 0.05
            onMoved: mask.brush()
        }
    }

    QQC2.Label {
        Layout.fillWidth: true
        visible: mask.painting
        wrapMode: Text.Wrap
        text: App.tr("Paint on the layer in the preview; each stroke is one step to undo.")
    }
}
